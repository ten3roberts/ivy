use std::{
    any::{Any, TypeId},
    collections::BTreeMap,
    sync::LazyLock,
};

use ivy_assets::Resource;
use serde::{
    de::{self, DeserializeOwned, DeserializeSeed, Visitor},
    ser::SerializeMap,
    Deserialize, Serialize, Serializer,
};

use crate::{template::BundleDesc, Bundle};

type DeserializeFn =
    fn(&mut dyn erased_serde::Deserializer) -> erased_serde::Result<Box<dyn BundleDesc>>;
type SerializeFn =
    fn(&dyn BundleDesc, &mut dyn erased_serde::Serializer) -> erased_serde::Result<()>;

/// Statically typed registered bundle
#[derive(Clone, Copy)]
pub struct BundleRegistration {
    pub tag: &'static str,
    pub type_id: fn() -> TypeId,
    pub desc_type_id: fn() -> TypeId,
    pub deserialize_fn: DeserializeFn,
    pub serialize_fn: SerializeFn,
    pub upcast_any: fn(Box<dyn Send + Sync + Any>) -> Box<dyn BundleDesc>,
}

impl BundleRegistration {
    pub const fn new<T>(tag: &'static str) -> Self
    where
        T: Bundle + Resource,
        T::Desc: BundleDesc + Serialize + DeserializeOwned,
    {
        Self {
            tag,
            type_id: || TypeId::of::<T>(),
            desc_type_id: || TypeId::of::<T::Desc>(),
            deserialize_fn: |de| {
                erased_serde::deserialize::<T::Desc>(de).map(|v| Box::new(v) as Box<dyn BundleDesc>)
            },
            serialize_fn: |this, serializer| {
                let this = this.downcast_ref::<T::Desc>().unwrap();
                erased_serde::Serialize::erased_serialize(this, serializer)
            },
            upcast_any: |v| {
                let v = v.downcast::<T::Desc>().expect("Invalid bundle type");
                Box::new(*v)
            },
        }
    }
}

/// Registry of available bundles, used for editing and serialization.
///
/// **Note**:, due to loading requirements, bundles are serialized and deserialized as their
/// description, as a bundle may contain live data, such as sound tracks or image data.
pub struct BundleRegistry {
    bundles: BTreeMap<&'static str, BundleRegistration>,
    names: Vec<&'static str>,
}

impl BundleRegistry {
    pub fn bundles(&self) -> &BTreeMap<&'static str, BundleRegistration> {
        &self.bundles
    }

    pub fn by_tag(&self, tag: &str) -> Option<&BundleRegistration> {
        self.bundles.get(tag)
    }
}

struct BundleLookupVisitor<'a> {
    registry: &'a BundleRegistry,
}

impl Visitor<'_> for BundleLookupVisitor<'static> {
    type Value = &'static BundleRegistration;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("an externally tagged type")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        match self.registry.bundles.get(v) {
            Some(v) => Ok(v),
            None => Err(de::Error::unknown_variant(v, &self.registry.names)),
        }
    }
}

impl<'de> DeserializeSeed<'de> for BundleLookupVisitor<'static> {
    type Value = &'static BundleRegistration;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        deserializer.deserialize_str(self)
    }
}

pub struct TaggedBundleVisitor {
    registry: &'static BundleRegistry,
}

impl<'de> Visitor<'de> for TaggedBundleVisitor {
    type Value = Box<dyn BundleDesc>;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("an externally tagged bundle")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: de::MapAccess<'de>,
    {
        let key = map.next_key_seed(BundleLookupVisitor {
            registry: self.registry,
        })?;

        let Some(key) = key else {
            return Err(de::Error::custom(format_args!(
                "expected externally tagged bundle",
            )));
        };

        let value = map.next_value_seed(DeserializeWithFunction {
            func: key.deserialize_fn,
        })?;

        Ok(value)
    }
}

struct DeserializeWithFunction {
    func: DeserializeFn,
}

impl<'de> DeserializeSeed<'de> for DeserializeWithFunction {
    type Value = Box<dyn BundleDesc>;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        let mut erased = <dyn erased_serde::Deserializer>::erase(deserializer);
        (self.func)(&mut erased).map_err(de::Error::custom)
    }
}

pub static BUNDLE_REGISTRY: LazyLock<BundleRegistry> = LazyLock::new(|| {
    let bundles = inventory::iter::<BundleRegistration>();
    let bundles = bundles
        .map(|v| (v.tag, *v))
        .collect::<BTreeMap<&'static str, _>>();

    let names = bundles.keys().copied().collect::<Vec<&'static str>>();

    BundleRegistry { bundles, names }
});

impl Serialize for dyn BundleDesc {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut ser = serializer.serialize_map(Some(1))?;

        ser.serialize_entry(self.tag_name(), &WrapErased(self))?;
        ser.end()
    }
}

struct WrapErased<'a, T: ?Sized>(pub &'a T);

impl<'a, T> Serialize for WrapErased<'a, T>
where
    T: ?Sized + BundleDesc + 'a + Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        erased_serde::serialize(self.0, serializer)
    }
}

impl<'de> Deserialize<'de> for Box<dyn BundleDesc> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        deserializer.deserialize_map(TaggedBundleVisitor {
            registry: &BUNDLE_REGISTRY,
        })
    }
}

/// Register a loadable bundle to the global [`BundleRegistry`]
///
/// **NOTE**: Only applicable for resource bundles that can be loaded from the assets system.
#[macro_export]
macro_rules! register_bundle {
    ($ty: ty) => {
        $crate::bundle_registry::__private::inventory::submit! {
            $crate::bundle_registry::BundleRegistration::new::<$ty>(stringify!($ty))
        }
    };
}

inventory::collect!(BundleRegistration);

#[doc(hidden)]
pub mod __private {
    pub use inventory;
}

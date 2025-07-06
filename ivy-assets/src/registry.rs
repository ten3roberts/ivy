use std::{any::Any, collections::BTreeMap, sync::LazyLock};

pub(crate) type DeserializeFn =
    fn(&mut dyn erased_serde::Deserializer) -> erased_serde::Result<Box<dyn LoadableDyn>>;
type SerializeFn = fn(&dyn LoadableDyn) -> &dyn erased_serde::Serialize;

/// Global registry of all implementors of [`Resource`].
pub struct ResourceRegistry {
    resources: BTreeMap<&'static str, ResourceRegistration>,
}

impl ResourceRegistry {
    pub fn new() -> Self {
        let resources = inventory::iter::<ResourceRegistration>()
            .map(|v| (v.type_name, v.clone()))
            .collect();

        Self { resources }
    }

    pub fn get(&self, ty: &str) -> Option<&ResourceRegistration> {
        self.resources.get(ty)
    }
}

pub static RESOURCE_REGISTRY: LazyLock<ResourceRegistry> = LazyLock::new(ResourceRegistry::new);

#[derive(Clone, Debug)]
pub struct ResourceRegistration {
    pub type_name: &'static str,
    pub deserialize_fn: DeserializeFn,
    pub serialize_fn: SerializeFn,
}

impl ResourceRegistration {
    pub const fn new<T: Resource>(type_name: &'static str) -> Self
    where
        T::Desc: serde::Serialize + DeserializeOwned,
        T::Desc: LoadableDyn + 'static,
    {
        Self {
            type_name,
            deserialize_fn: |de| {
                erased_serde::deserialize::<T::Desc>(de)
                    .map(|v| Box::new(v) as Box<dyn LoadableDyn>)
            },
            serialize_fn: |value| {
                let value = value
                    .downcast_ref::<T::Desc>()
                    .expect("Failed to downcast LoadableDyn");

                value
            },
        }
    }
}

inventory::collect!(ResourceRegistration);

#[doc(hidden)]
pub use inventory;

#[doc(hidden)]
pub use serde;

use serde::de::DeserializeOwned;

use crate::{loadable::LoadableDyn, Resource};

#[macro_export]
macro_rules! declare_resource {
    ($name:ident, $desc:ty) => {
        $crate::registry::inventory::submit! {
            $crate::registry::ResourceRegistration::new::<$name>(stringify!($name))
        }

        impl Resource for $name {
            type Desc = $desc;

            fn type_name() -> &'static str {
                stringify!($name)
            }
        }
    };
}

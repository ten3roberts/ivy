use anyhow::Context;
use serde::{
    de::{self, DeserializeSeed, Visitor},
    ser::SerializeStruct,
    Deserialize, Serialize,
};

use crate::{
    loadable::{LoadFromPath, LoadableDyn},
    registry::{DeserializeFn, RESOURCE_REGISTRY},
    AssetCache, AssetPath,
};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AssetMeta {
    pub type_name: String,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct AssetPayload<T> {
    pub meta: AssetMeta,
    pub desc: T,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct AssetPayloadMeta {
    pub meta: AssetMeta,
}

pub struct AssetPayloadUntyped {
    pub meta: AssetMeta,
    pub desc: Box<dyn LoadableDyn>,
}

impl AssetPayloadUntyped {
    pub fn new(meta: AssetMeta, desc: Box<dyn LoadableDyn>) -> Self {
        Self { meta, desc }
    }

    pub fn serialize_json(&self) -> anyhow::Result<String> {
        tracing::info!("Serializing asset: {}", self.meta.type_name);
        serde_json::to_string_pretty(self)
            .with_context(|| format!("Failed to serialize asset: {}", self.meta.type_name))
    }

    pub async fn load_meta_from_file(
        path: &AssetPath<Self>,
        assets: &AssetCache,
    ) -> anyhow::Result<AssetMeta> {
        let content = path.load_file_content(assets).await?;

        let payload: AssetPayloadMeta = serde_json::from_slice(&content[..])?;
        Ok(payload.meta)
    }
}

impl<'de> Deserialize<'de> for AssetPayloadUntyped {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        deserializer.deserialize_map(PayloadVisitor)
    }
}

impl AssetPayloadUntyped {
    pub fn meta(&self) -> &AssetMeta {
        &self.meta
    }

    pub fn desc(&self) -> &Box<dyn LoadableDyn> {
        &self.desc
    }
}

impl LoadFromPath for AssetPayloadUntyped {
    async fn load_from_file(path: AssetPath<Self>, assets: &AssetCache) -> anyhow::Result<Self>
    where
        Self: Sized,
    {
        let content = assets
            .service::<crate::service::FileSystemMapService>()
            .load_bytes_async(path.path())
            .await?;

        let payload: AssetPayloadUntyped = serde_json::from_slice(&content[..])?;

        Ok(payload)
    }
}

struct PayloadVisitor;

impl<'de> Visitor<'de> for PayloadVisitor {
    type Value = AssetPayloadUntyped;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("struct of meta and asset value")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::MapAccess<'de>,
    {
        let mut meta: Option<AssetMeta> = None;
        let mut desc: Option<Box<dyn LoadableDyn>> = None;

        while let Some(key) = map.next_key()? {
            match key {
                PayloadField::Meta => {
                    if meta.is_some() {
                        return Err(serde::de::Error::duplicate_field("meta"));
                    }
                    meta = Some(map.next_value()?);
                }
                PayloadField::Desc => {
                    if desc.is_some() {
                        return Err(serde::de::Error::duplicate_field("desc"));
                    }

                    match &meta {
                        Some(meta) => {
                            // sweet, meta available, we can deserialize the type directly
                            let deserialize_fn = RESOURCE_REGISTRY
                                .get(&meta.type_name)
                                .ok_or_else(|| serde::de::Error::custom("Unknown type name"))?
                                .deserialize_fn;

                            desc = Some(map.next_value_seed(DeserializeWithFunction {
                                func: deserialize_fn,
                            })?);
                        }
                        None => {
                            // Darn, buffer content
                            todo!()
                        }
                    }
                }
            }
        }

        let meta = meta.ok_or_else(|| serde::de::Error::missing_field("meta"))?;
        let desc = desc.ok_or_else(|| serde::de::Error::missing_field("desc"))?;

        Ok(AssetPayloadUntyped { meta, desc })
    }
}

impl Serialize for AssetPayloadUntyped {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("AssetPayload", 2)?;

        let serialize_fn = RESOURCE_REGISTRY
            .get(&self.meta.type_name)
            .ok_or_else(|| serde::ser::Error::custom("Unknown type name"))?
            .serialize_fn;

        let desc = serialize_fn(&*self.desc);
        state.serialize_field("meta", &self.meta)?;
        state.serialize_field("desc", desc)?;
        state.end()
    }
}

#[derive(serde::Deserialize)]
#[serde(field_identifier, rename_all = "snake_case")]
enum PayloadField {
    Meta,
    Desc,
}

struct DeserializeWithFunction {
    func: DeserializeFn,
}

impl<'de> DeserializeSeed<'de> for DeserializeWithFunction {
    type Value = Box<dyn LoadableDyn>;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        let mut erased = <dyn erased_serde::Deserializer>::erase(deserializer);
        (self.func)(&mut erased).map_err(de::Error::custom)
    }
}

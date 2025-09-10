use std::io::Read;

use anyhow::Context;
use flax::{
    fetch::entity_refs, serialize::SerializationContext, Entity, EntityBuilder, EntityRef, Query,
    World,
};
use futures::{stream, StreamExt, TryStreamExt};
use ivy_assets::{loadable::Loadable, Asset, AssetCache, AssetPath, AsyncAssetExt};
use ivy_core::template::{template_key, Template};
use serde::{
    de::{self, DeserializeSeed, Visitor},
    ser::{self, SerializeMap, SerializeSeq},
    Serialize,
};

/// Allows serializing and deserializing a saved scene.
///
/// # Format
///
/// A scene is described as a collection of entities.
///
/// Each Entity consists of an originating [`Template`][`ivy_core::template::Template`], and a
/// collection of serialized component data, such as positions.
pub struct SceneSerializer {
    serialization_context: SerializationContext,
}

impl Default for SceneSerializer {
    fn default() -> Self {
        Self::new()
    }
}

impl SceneSerializer {
    pub fn new() -> Self {
        Self {
            serialization_context: SerializationContext::builder().from_registry().build(),
        }
    }

    /// Serializes an entire world
    pub fn serialize_scene<'a>(&'a self, world: &'a World) -> SerializeScene<'a> {
        SerializeScene { ctx: self, world }
    }

    pub fn serialize_entity<'a>(&'a self, entity: &'a EntityRef) -> SerializeEntity<'a> {
        SerializeEntity { ctx: self, entity }
    }

    fn filter_entity(&self, entity: &EntityRef) -> bool {
        entity.has(template_key())
    }

    /// Serializes a world to json
    pub fn serialize_json(
        &self,
        world: &World,
        writer: impl std::io::Write,
    ) -> Result<(), serde_json::Error> {
        let mut serializer = serde_json::Serializer::pretty(writer);
        self.serialize_scene(world).serialize(&mut serializer)
    }

    /// Deserializes a scene from json
    pub async fn deserialize_json(
        &self,
        assets: &AssetCache,
        reader: impl Read,
    ) -> anyhow::Result<Scene> {
        let mut deserializer = serde_json::Deserializer::from_reader(reader);
        let value = SceneDeserializer { ctx: self }.deserialize(&mut deserializer)?;

        let scene = value.build(assets).await?;
        Ok(scene)
    }
}

pub struct SerializeScene<'a> {
    ctx: &'a SceneSerializer,
    world: &'a World,
}

impl Serialize for SerializeScene<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut s = serializer.serialize_map(Some(1))?;

        s.serialize_entry(
            "entities",
            &EntitiesSerializer {
                ctx: self.ctx,
                world: self.world,
            },
        )?;

        s.end()
    }
}

struct EntitiesSerializer<'a> {
    ctx: &'a SceneSerializer,
    world: &'a World,
}

impl Serialize for EntitiesSerializer<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut entities = Query::new(entity_refs()).with(template_key());

        let count = entities.borrow(self.world).count();

        let mut s = serializer.serialize_seq(Some(count))?;
        for entity in &mut entities.borrow(self.world) {
            if !self.ctx.filter_entity(&entity) {
                continue;
            }

            s.serialize_element(&SceneEntitySerializer {
                ctx: self.ctx,
                entity: &entity,
            })?;
        }

        s.end()
    }
}

pub struct SerializeEntity<'a> {
    ctx: &'a SceneSerializer,
    entity: &'a EntityRef<'a>,
}

impl Serialize for SerializeEntity<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut s = serializer.serialize_map(Some(3))?;

        let template = self
            .entity
            .get_clone(template_key())
            .map_err(|_| serde::ser::Error::custom("missing template on serialized entity"))?;

        s.serialize_entry("template", &template)?;
        s.serialize_entry(
            "components",
            &self.ctx.serialization_context.serialize_entity(self.entity),
        )?;

        s.end()
    }
}

struct SceneEntitySerializer<'a> {
    ctx: &'a SceneSerializer,
    entity: &'a EntityRef<'a>,
}

impl Serialize for SceneEntitySerializer<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        let mut s = serializer.serialize_map(Some(3))?;

        let template = self.entity.get_clone(template_key()).ok();

        s.serialize_entry("id", &self.entity.id())?;
        s.serialize_entry("template", &template)?;
        s.serialize_entry(
            "data",
            &self.ctx.serialization_context.serialize_entity(self.entity),
        )?;

        s.end()
    }
}

struct SceneEntityDeserializer<'a> {
    ctx: &'a SceneSerializer,
}

impl<'de> Visitor<'de> for SceneEntityDeserializer<'_> {
    type Value = SceneEntityDesc;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("struct SceneEntity")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: de::MapAccess<'de>,
    {
        let mut id: Option<Entity> = None;
        let mut template: Option<AssetPath<Template>> = None;
        let mut data: Option<EntityBuilder> = None;

        while let Some(field) = map.next_key::<SceneEntityField>()? {
            match field {
                SceneEntityField::Id => id = Some(map.next_value()?),
                SceneEntityField::Template => template = Some(map.next_value()?),
                SceneEntityField::Data => {
                    data = Some(
                        map.next_value_seed(self.ctx.serialization_context.deserialize_entity())?,
                    )
                }
            }
        }

        let id = id.ok_or_else(|| de::Error::missing_field("id"))?;
        let template = template.ok_or_else(|| de::Error::missing_field("template"))?;
        let data = data.ok_or_else(|| de::Error::missing_field("data"))?;

        Ok(SceneEntityDesc { id, template, data })
    }
}

impl<'de> DeserializeSeed<'de> for SceneEntityDeserializer<'_> {
    type Value = SceneEntityDesc;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        deserializer.deserialize_struct("SceneEntity", &["id", "template", "data"], self)
    }
}

struct SceneEntitiesDeserializer<'a> {
    ctx: &'a SceneSerializer,
}

impl<'de> Visitor<'de> for SceneEntitiesDeserializer<'_> {
    type Value = SceneDesc;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("a sequence of entities")
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: de::SeqAccess<'de>,
    {
        let mut entities = Vec::new();
        while let Some(element) =
            seq.next_element_seed(SceneEntityDeserializer { ctx: self.ctx })?
        {
            entities.push(element);
        }

        Ok(SceneDesc { entities })
    }
}

impl<'de> DeserializeSeed<'de> for SceneEntitiesDeserializer<'_> {
    type Value = SceneDesc;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        deserializer.deserialize_seq(self)
    }
}

struct SceneDeserializer<'a> {
    ctx: &'a SceneSerializer,
}

pub struct SceneDesc {
    entities: Vec<SceneEntityDesc>,
}

pub struct Scene {
    pub world: World,
}

impl SceneDesc {
    async fn build(self, assets: &AssetCache) -> anyhow::Result<Scene> {
        let mut world = World::new();

        for entity in &self.entities {
            world
                .spawn_at(entity.id)
                .context("Duplicate entities in save")?;
        }

        fn clean_entity_builders(entity: &mut EntityBuilder) {
            // remove all children containing a template key
            entity.children_mut().retain_mut(|child| {
                if child.has(template_key()) {
                    clean_entity_builders(child);
                    false
                } else {
                    true
                }
            });
        }

        stream::iter(self.entities.into_iter())
            .map(|v| async {
                let id = v.id;
                v.load(assets)
                    .await
                    .with_context(|| format!("Failed to load entity {:?}", id))
            })
            .buffered(8)
            .boxed()
            .try_for_each(|mut entity| {
                let mut entity_data = entity.template.build();

                clean_entity_builders(&mut entity_data);
                entity_data.append(&mut entity.data);

                let result = entity_data.append_to(&mut world, entity.id);

                async { result.map(|_| {}).map_err(anyhow::Error::from) }
            })
            .await?;

        Ok(Scene { world })
    }
}

impl<'de> Visitor<'de> for SceneDeserializer<'_> {
    type Value = SceneDesc;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("struct Scene")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::MapAccess<'de>,
    {
        let mut entities = None;
        while let Some(field) = map.next_key::<SceneField>()? {
            match field {
                SceneField::Entities => {
                    entities =
                        Some(map.next_value_seed(SceneEntitiesDeserializer { ctx: self.ctx })?)
                }
            }
        }

        let entities = entities.ok_or_else(|| de::Error::missing_field("entities"))?;

        Ok(entities)
    }
}

impl<'de> DeserializeSeed<'de> for SceneDeserializer<'_> {
    type Value = SceneDesc;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_map(self)
    }
}

pub struct SceneEntityDesc {
    id: Entity,
    template: AssetPath<Template>,
    data: EntityBuilder,
}

impl SceneEntityDesc {
    async fn load(self, assets: &AssetCache) -> anyhow::Result<SceneEntity> {
        Ok(SceneEntity {
            id: self.id,
            template: self
                .template
                .load_async(assets)
                .await
                .with_context(|| format!("Failed to load template {:?}", self.template))?,
            data: self.data,
        })
    }
}

pub struct SceneEntity {
    id: Entity,
    template: Asset<Template>,
    data: EntityBuilder,
}

#[derive(serde::Deserialize)]
#[serde(field_identifier, rename_all = "lowercase")]
enum SceneField {
    Entities,
}

#[derive(serde::Deserialize)]
#[serde(field_identifier, rename_all = "lowercase")]
enum SceneEntityField {
    Id,
    Template,
    Data,
}

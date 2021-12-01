use crate::{Error, Result};
use std::{borrow::Cow, collections::HashMap};

use derive_more::{From, Into};
use hecs::{Component, DynamicBundle, Entity, EntityBuilderClone, World};

//// Generic container for storing entity templates for later retrieval and
///spawning. Intended to be stored inside resources or standalone.
pub struct TemplateStore {
    templates: HashMap<TemplateKey, Box<dyn Template>>,
}

impl TemplateStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts a new template. A template is anything that is a closure or a
    /// built cloneable entity.
    pub fn insert<T: Template>(&mut self, key: TemplateKey, template: T) {
        self.templates.insert(key, Box::new(template));
    }

    /// Returns the template associated by key.
    pub fn get(&self, key: &TemplateKey) -> Result<&dyn Template> {
        self.templates
            .get(key)
            .map(|val| val.as_ref())
            .ok_or_else(|| Error::InvalidTemplateKey(key.clone()))
    }

    /// Returns the template associated by key.
    pub fn get_mut<'a>(&'a mut self, key: &TemplateKey) -> Result<&'a mut dyn Template> {
        self.templates
            .get_mut(key)
            .map(|val| val.as_mut())
            .ok_or_else(|| Error::InvalidTemplateKey(key.clone()))
    }

    /// Spawns a template by key into the world.
    pub fn spawn(&self, world: &mut World, key: &TemplateKey) -> Result<Entity> {
        Ok(self.get(key)?.spawn(world))
    }

    //     /// Builds a template and returns it
    pub fn builder(&self, key: &TemplateKey) -> Result<EntityBuilderClone> {
        Ok(self.get(key)?.builder())
    }

    /// Spawns a template by key into the world and assigns the provided bundle.
    pub fn spawn_with<T: DynamicBundle>(
        &mut self,
        world: &mut World,
        key: &TemplateKey,
        bundle: T,
    ) -> Result<Entity> {
        let entity = self.get_mut(key)?.spawn(world);
        world
            .insert(entity, bundle)
            .expect("Failed to insert component into newly spawned entity");
        Ok(entity)
    }
}

impl Default for TemplateStore {
    fn default() -> Self {
        Self {
            templates: Default::default(),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, PartialOrd, Ord, Default, Hash, Into, From)]
pub struct TemplateKey(Cow<'static, str>);

impl TemplateKey {
    pub fn new<S: Into<Cow<'static, str>>>(name: S) -> Self {
        Self(name.into())
    }
}

impl std::ops::Deref for TemplateKey {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}

impl std::ops::DerefMut for TemplateKey {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.to_mut()
    }
}

impl From<&'static str> for TemplateKey {
    fn from(val: &'static str) -> Self {
        Self::new(val)
    }
}

pub trait Template: Component {
    fn spawn<'a>(&self, world: &'a mut World) -> Entity;
    fn builder(&self) -> EntityBuilderClone;
}

impl<F: Fn() -> EntityBuilderClone + Component> Template for F {
    fn spawn<'a>(&self, world: &'a mut World) -> Entity {
        world.spawn(&self.builder().build())
    }

    fn builder(&self) -> EntityBuilderClone {
        (self)()
    }
}

impl Template for EntityBuilderClone {
    fn spawn<'a>(&self, world: &'a mut World) -> Entity {
        world.spawn(&self.clone().build())
    }

    fn builder(&self) -> EntityBuilderClone {
        self.clone()
    }
}

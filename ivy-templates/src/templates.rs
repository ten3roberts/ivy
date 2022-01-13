use crate::{Error, Result, Template, TemplateKey};
use std::collections::HashMap;

use hecs::{DynamicBundleClone, Entity, EntityBuilderClone, World};

use hecs_schedule::{CommandBuffer, GenericWorld};

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
    pub fn insert<T: Template>(&mut self, key: TemplateKey, mut template: T) {
        template.root_mut().add(key.clone());
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
    pub fn spawn(
        &self,
        world: &mut World,
        key: &TemplateKey,
        extra: impl DynamicBundleClone,
    ) -> Result<Entity> {
        let template = self.get(key)?;
        let mut builder = EntityBuilderClone::new();
        builder.add_bundle(extra);
        Ok(template.build(world, builder))
    }

    /// Spawns a template by key into the world.
    pub fn spawn_deferred<'a>(
        &self,
        world: &impl GenericWorld,
        cmd: &mut CommandBuffer,
        key: &TemplateKey,
        extra: impl DynamicBundleClone,
    ) -> Result<Entity> {
        let template = self.get(key)?;

        let mut builder = EntityBuilderClone::new();
        builder.add_bundle(extra);
        let e = template.build_cmd(&world.into_empty(), cmd, builder);
        Ok(e)
    }
}

impl Default for TemplateStore {
    fn default() -> Self {
        Self {
            templates: Default::default(),
        }
    }
}

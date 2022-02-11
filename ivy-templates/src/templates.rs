use crate::{Error, Result, TemplateKey};
use std::collections::HashMap;

use hecs::{DynamicBundleClone, Entity, World};

use hecs_hierarchy::TreeBuilderClone;
use hecs_schedule::{CommandBuffer, GenericWorld};
use ivy_base::Connection;

pub type Template = TreeBuilderClone<Connection>;

//// Generic container for storing entity templates for later retrieval and
///spawning. Intended to be stored inside resources or standalone.
pub struct TemplateStore {
    templates: HashMap<TemplateKey, Template>,
}

impl TemplateStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts a new template. A template is anything that is a closure or a
    /// built cloneable entity.
    pub fn insert(&mut self, key: TemplateKey, template: impl Into<Template>) {
        let mut template = template.into();
        template.add(key.clone());
        self.templates.insert(key, template);
    }

    /// Returns the template associated by key.
    pub fn get(&self, key: &TemplateKey) -> Result<&Template> {
        self.templates
            .get(key)
            .ok_or_else(|| Error::InvalidTemplateKey(key.clone()))
    }

    /// Returns the template associated by key.
    pub fn get_mut<'a>(&'a mut self, key: &TemplateKey) -> Result<&'a mut Template> {
        self.templates
            .get_mut(key)
            .ok_or_else(|| Error::InvalidTemplateKey(key.clone()))
    }

    /// Spawns a template by key into the world.
    pub fn spawn(
        &self,
        world: &mut World,
        key: &TemplateKey,
        extra: impl DynamicBundleClone,
    ) -> Result<Entity> {
        let mut template = self.get(key)?.clone();
        template.add_bundle(extra);
        Ok(template.spawn(world))
    }

    /// Spawns a template by key into the world.
    pub fn spawn_deferred<'a>(
        &self,
        world: &impl GenericWorld,
        cmd: &mut CommandBuffer,
        key: &TemplateKey,
        extra: impl DynamicBundleClone,
    ) -> Result<Entity> {
        let mut template = self.get(key)?.clone();

        template.add_bundle(extra);

        let e = template.reserve(world);
        cmd.write(move |w| {
            template.spawn(w);
        });
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

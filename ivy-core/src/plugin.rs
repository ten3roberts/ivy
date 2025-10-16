use anyhow::Result;
use flax::World;
use ivy_assets::{stored::DynamicStore, AssetCache};

use crate::update_layer::ScheduleSetBuilder;

pub struct PluginContext<'a> {
    pub world: &'a mut World,
    pub assets: &'a AssetCache,
    pub store: &'a mut DynamicStore,
    pub schedules: &'a mut ScheduleSetBuilder,
}

/// A plugin is added to a layer and allows logic to be added using the ECS
///
/// For full control of events and update frequency, use [crate::layer::Layer].
pub trait Plugin {
    // Installs the plugin to the schedule set
    fn install(&self, ctx: PluginContext) -> anyhow::Result<()>;

    fn key(&self) -> &'static str {
        std::any::type_name::<Self>()
    }

    // Plugin runs before other plugin
    fn before(&self) -> Vec<&str> {
        Vec::new()
    }

    // Plugin runs after another plugin
    fn after(&self) -> Vec<&str> {
        Vec::new()
    }
}

impl<U: Plugin> Plugin for Box<U> {
    fn install(&self, ctx: PluginContext) -> Result<(), anyhow::Error> {
        (**self).install(ctx)
    }
}
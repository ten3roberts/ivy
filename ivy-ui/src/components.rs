use flax::{component, World};
use ivy_assets::{stored::Handle, AssetCache};
use ivy_input::types::InputEvent;
use ivy_wgpu::rendergraph::TextureHandle;
use violet::{core::ScopeRef, wgpu::app::AppInstance};

component! {
    pub ui_instance: Handle<AppInstance>,
    pub texture_dependency: TextureHandle,
    pub on_input_event: Box<dyn Send + Sync + FnMut(&ScopeRef<'_>, &mut World, &AssetCache, &InputEvent) -> anyhow::Result<()>>,
}

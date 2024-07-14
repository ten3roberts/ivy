use flax::{components::child_of, Entity, EntityBuilder};
use ivy_assets::AssetCache;
use ivy_core::EntityBuilderExt;
use ivy_gltf::GltfNode;
use ivy_wgpu::{
    components::mesh_primitive,
    renderer::RenderObjectBundle,
    shaders::{PbrShaderDesc, SkinnedPbrShaderDesc},
};

pub trait GltfNodeExt {
    fn mount<'a>(
        &self,
        assets: &AssetCache,
        entity: &'a mut EntityBuilder,
    ) -> &'a mut EntityBuilder;
}

impl GltfNodeExt for GltfNode {
    fn mount<'a>(
        &self,
        assets: &AssetCache,
        entity: &'a mut EntityBuilder,
    ) -> &'a mut EntityBuilder {
        let skin = self.skin();

        if let Some(mesh) = self.mesh() {
            for primitive in mesh.primitives() {
                let material = primitive.material().into();
                entity.attach(
                    mesh_primitive,
                    Entity::builder().mount(RenderObjectBundle::new(
                        primitive.into(),
                        material,
                        if skin.is_some() {
                            assets.load(&SkinnedPbrShaderDesc)
                        } else {
                            assets.load(&PbrShaderDesc)
                        },
                    )),
                );
            }
        }

        if let Some(skin) = skin {
            tracing::info!("adding skin to node");
            entity.set(ivy_gltf::components::skin(), skin);
        }

        entity.mount(self.transform());

        for child in self.children() {
            entity.attach(child_of, child.mount(assets, &mut Entity::builder()));
        }

        entity
    }
}

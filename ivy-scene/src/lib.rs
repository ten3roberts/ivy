use flax::{components::child_of, Entity, EntityBuilder};
use ivy_assets::AssetCache;
use ivy_base::{EntityBuilderExt, TransformBundle};
use ivy_gltf::{GltfNode, GltfNodeRef};
use ivy_wgpu::{components::mesh_primitive, renderer::RenderObjectBundle, shaders::PbrShaderKey};

pub trait GltfNodeExt {
    fn mount<'a>(
        &self,
        assets: &AssetCache,
        entity: &'a mut EntityBuilder,
    ) -> &'a mut EntityBuilder;
}

impl GltfNodeExt for GltfNodeRef<'_> {
    fn mount<'a>(
        &self,
        assets: &AssetCache,
        entity: &'a mut EntityBuilder,
    ) -> &'a mut EntityBuilder {
        if let Some(mesh) = self.mesh() {
            for primitive in mesh.primitives() {
                let material = primitive.material().into();
                entity.attach(
                    mesh_primitive,
                    Entity::builder().mount(RenderObjectBundle::new(
                        primitive.into(),
                        material,
                        assets.load(&PbrShaderKey),
                    )),
                );
            }
        }

        let (position, rotation, scale) = self.transform();
        entity.mount(TransformBundle::new(position, rotation, scale));

        for child in self.children() {
            entity.attach(child_of, child.mount(assets, &mut Entity::builder()));
        }

        entity
    }
}

impl GltfNodeExt for GltfNode {
    fn mount<'a>(
        &self,
        assets: &AssetCache,
        entity: &'a mut EntityBuilder,
    ) -> &'a mut EntityBuilder {
        self.get_ref().mount(assets, entity)
    }
}

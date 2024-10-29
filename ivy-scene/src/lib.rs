use flax::{components::child_of, Component, Entity, EntityBuilder};
use ivy_assets::{Asset, AssetCache};
use ivy_core::EntityBuilderExt;
use ivy_gltf::GltfNode;
use ivy_wgpu::{
    components::{forward_pass, mesh_primitive, shadow_pass},
    renderer::RenderObjectBundle,
    shader::ShaderPass,
    shaders::{PbrShaderDesc, ShadowShaderDesc, SkinnedPbrShaderDesc, SkinnedShadowShaderDesc},
};

#[derive(Debug, Clone, Copy)]
pub struct NodeMountOptions {}

pub trait GltfNodeExt {
    fn mount<'a>(
        &self,
        assets: &AssetCache,
        entity: &'a mut EntityBuilder,
        opts: NodeMountOptions,
    ) -> &'a mut EntityBuilder;

    fn mount_with_shaders<'a>(
        &self,
        entity: &'a mut EntityBuilder,
        shaders: &[(Component<Asset<ShaderPass>>, Asset<ShaderPass>)],
        opts: NodeMountOptions,
    ) -> &'a mut EntityBuilder;
}

impl GltfNodeExt for GltfNode {
    fn mount<'a>(
        &self,
        assets: &AssetCache,
        entity: &'a mut EntityBuilder,
        opts: NodeMountOptions,
    ) -> &'a mut EntityBuilder {
        let shader;
        let shadow_shader;

        match self.skin() {
            Some(_) => {
                shader = assets.load(&SkinnedPbrShaderDesc);
                shadow_shader = assets.load(&SkinnedShadowShaderDesc);
            }
            None => {
                shader = assets.load(&PbrShaderDesc);
                shadow_shader = assets.load(&ShadowShaderDesc);
            }
        }

        self.mount_with_shaders(
            entity,
            &[(forward_pass(), shader), (shadow_pass(), shadow_shader)],
            opts,
        )
    }

    fn mount_with_shaders<'a>(
        &self,
        entity: &'a mut EntityBuilder,
        shaders: &[(Component<Asset<ShaderPass>>, Asset<ShaderPass>)],
        opts: NodeMountOptions,
    ) -> &'a mut EntityBuilder {
        let skin = self.skin();

        if let Some(mesh) = self.mesh() {
            for primitive in mesh.primitives() {
                let material = primitive.material().into();

                let mut child = Entity::builder();

                child.mount(RenderObjectBundle::new(primitive.into(), material, shaders));

                entity.attach(mesh_primitive, child);
            }
        }

        if let Some(skin) = skin {
            tracing::info!("adding skin to node");
            entity.set(ivy_gltf::components::skin(), skin);
        }

        tracing::info!(determinant = self.transform_matrix().determinant());
        entity.mount(self.transform());

        for child in self.children() {
            entity.attach(
                child_of,
                child.mount_with_shaders(&mut Entity::builder(), shaders, opts),
            );
        }

        entity
    }
}

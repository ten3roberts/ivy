use std::collections::BTreeMap;

use flax::{
    components::{child_of, name},
    Entity, EntityBuilder,
};
use ivy_core::EntityBuilderExt;
use ivy_gltf::{animation::player::Animator, components::animator, GltfNode};
use ivy_wgpu::{
    components::{forward_pass, shadow_pass},
    material_desc::{MaterialData, PbrMaterialData},
    renderer::RenderObjectBundle,
};

#[derive(Debug, Clone, Copy)]
pub struct NodeMountOptions<'a> {
    pub skip_empty_children: bool,
    pub material_overrides: &'a BTreeMap<String, MaterialData>,
}

pub trait GltfNodeExt {
    fn mount<'a>(
        &self,
        entity: &'a mut EntityBuilder,
        opts: &NodeMountOptions,
    ) -> &'a mut EntityBuilder;
}

impl GltfNodeExt for GltfNode {
    fn mount<'a>(
        &self,
        entity: &'a mut EntityBuilder,
        opts: &NodeMountOptions,
    ) -> &'a mut EntityBuilder {
        let skin = self.skin();

        if let Some(mesh) = self.mesh() {
            for primitive in mesh.primitives() {
                let gltf_material = primitive.material();

                let material = gltf_material
                    .name()
                    .and_then(|name| opts.material_overrides.get(name).cloned())
                    .unwrap_or_else(|| {
                        MaterialData::PbrMaterial(PbrMaterialData::from_gltf_material(
                            gltf_material,
                        ))
                    });

                let materials = [
                    (forward_pass(), material),
                    (shadow_pass(), MaterialData::ShadowMaterial),
                ];

                let mut child = Entity::builder();

                child
                    .mount(RenderObjectBundle::new(primitive.into(), &materials))
                    .set_opt(name(), mesh.name().map(ToOwned::to_owned));

                entity.attach(child_of, child);
            }
        }

        if let Some(skin) = skin {
            entity.set(ivy_gltf::components::skin(), skin);
            entity.set(animator(), Animator::new());
        }

        entity.mount(self.transform());

        for child in self.children() {
            if child.children().next().is_none() && child.mesh().is_none() {
                continue;
            }

            entity.attach(child_of, child.mount(&mut Entity::builder(), opts));
        }

        entity
    }
}

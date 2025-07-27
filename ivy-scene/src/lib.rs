pub mod camera;
mod collider;
pub mod editor;
pub mod ray_picker;

use std::collections::BTreeMap;

use flax::{
    components::{child_of, name},
    query::Node,
    Entity, EntityBuilder,
};
use glam::Mat4;
use ivy_core::{components::color, Bundle, Color, ColorExt, EntityBuilderExt};
use ivy_derive::Resource;
use ivy_editable::Editable;
use ivy_gltf::GltfNode;
use ivy_wgpu::{
    components::{forward_pass, shadow_pass},
    effect_desc::{PbrRenderEffect, RenderEffect},
    renderer::RenderObjectBundle,
};

pub use collider::*;

#[derive(Debug)]
pub struct NodeMountOptions<'a> {
    pub skip_empty_children: bool,
    pub material_overrides: &'a BTreeMap<String, RenderEffect>,
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
        fn mount<'a>(
            node: &GltfNode,
            entity: &'a mut EntityBuilder,
            opts: &NodeMountOptions,
        ) -> &'a mut EntityBuilder {
            let skin = node.skin();

            if let Some(mesh) = node.mesh() {
                for primitive in mesh.primitives() {
                    let gltf_material = primitive.material();

                    let forward_effect = gltf_material
                        .name()
                        .and_then(|name| opts.material_overrides.get(name).cloned())
                        .unwrap_or_else(|| {
                            RenderEffect::Pbr(PbrRenderEffect::from_gltf_material(gltf_material))
                        });

                    let effects = [
                        (forward_pass(), forward_effect),
                        (shadow_pass(), RenderEffect::OpaqueShadow),
                    ];

                    let mut child = Entity::builder();

                    child
                        .mount(RenderObjectBundle::new(primitive.into(), &effects))
                        .set_opt(name(), mesh.name().map(ToOwned::to_owned));

                    entity.attach(child_of, child);
                }
            }

            if let Some(skin) = skin {
                entity.set(
                    ivy_gltf::components::skin_matrix(),
                    vec![Mat4::IDENTITY; skin.joints().len()],
                );
                entity.set(ivy_gltf::components::skin(), skin);
            }

            entity.mount(node.transform()).set(color(), Color::white());

            for child in node.children() {
                if child.children().next().is_none() && child.mesh().is_none() {
                    continue;
                }

                entity.attach(child_of, mount(&child, &mut Entity::builder(), opts));
            }

            entity
        }

        mount(self, entity, opts)
    }
}

#[derive(Clone, Bundle, Resource)]
#[resource(derive = [Editable])]
pub struct NodeBundle {
    #[resource(load)]
    node: GltfNode,
}

impl Bundle for NodeBundle {
    fn mount(&self, entity: &mut EntityBuilder) {
        self.node.mount(
            entity,
            &NodeMountOptions {
                skip_empty_children: true,
                material_overrides: &Default::default(),
            },
        );
    }
}

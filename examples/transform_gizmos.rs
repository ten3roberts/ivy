use std::{convert::Infallible, f32::consts::TAU};

use anyhow::Context;
use flax::{
    components::{child_of, name},
    Entity, World,
};
use glam::{vec2, vec3, Quat, Vec3};
use image::Rgba;
use itertools::Itertools;
use ivy_assets::{
    fs::AssetPath, loadable::ResourceDesc, stored::DynamicStore, Asset, AssetCache, AssetDesc,
    AsyncAssetExt,
};
use ivy_core::{
    app::PostInitEvent,
    layer::events::EventRegisterContext,
    math::Axis3D,
    palette::Srgb,
    profiling::ProfilingLayer,
    transforms::TransformUpdatePlugin,
    update_layer::{FixedTimeStep, Plugin, ScheduledLayer},
    App, AsyncCommandBuffer, EngineLayer, EntityBuilderExt, Layer, DEG_45,
};
use ivy_engine::{async_commandbuffer, engine, RigidBodyBundle, TransformBundle};
use ivy_game::{
    orbit_camera::OrbitCameraPlugin,
    viewport_camera::{CameraSettings, ViewportCameraLayer},
};
use ivy_gltf::{
    animation::{
        player::{AnimationPlayer, Animator},
        AnimationDesc,
    },
    Document,
};
use ivy_graphics::{
    mesh::{MeshData, TANGENT_ATTRIBUTE},
    texture::TextureData,
};
use ivy_input::layer::InputLayer;
use ivy_physics::{ColliderBundle, GizmoSettings, PhysicsPlugin};
use ivy_postprocessing::preconfigured::{
    pbr::{PbrRenderGraphConfig, SkyboxConfig},
    SurfacePbrPipelineDesc, SurfacePbrRenderer,
};
use ivy_scene::{editor::hierarchy_panel::HierarchyPanel, GltfNodeExt, NodeMountOptions};
use ivy_ui::{
    layer::{UiLayer, UiUpdateLayer},
    screens::{screen_state, Screen, ScreenLifetimeToken, ScreenState},
    streamed::StreamedUiPlugin,
};
use ivy_wgpu::{
    components::{forward_pass, transparent_pass},
    driver::WinitDriver,
    layer::GraphicsLayer,
    material_desc::{MaterialData, PbrMaterialData},
    mesh_desc::MeshDesc,
    primitives::{CapsulePrimitive, CubePrimitive},
    renderer::{EnvironmentData, RenderObjectBundle},
};
use rapier3d::prelude::SharedShape;
use tracing_subscriber::{layer::SubscriberExt, registry, util::SubscriberInitExt, EnvFilter};
use tracing_tree::HierarchicalLayer;
use violet::{
    core::{
        components::LayoutAlignment,
        layout::Align,
        style::SizeExt,
        widget::{label, maximized},
        Widget,
    },
    palette::Srgba,
};
use wgpu::TextureFormat;
use winit::{dpi::LogicalSize, window::WindowAttributes};

const ENABLE_SKYBOX: bool = true;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ArrowMesh;

impl AssetDesc for ArrowMesh {
    type Output = MeshData;
    type Error = Infallible;

    fn create(&self, assets: &AssetCache) -> Result<Asset<Self::Output>, Self::Error> {
        const HEAD_LENGTH: f32 = 0.2;
        const HEAD_RADIUS: f32 = 0.05;
        const BASE_RADIUS: f32 = 0.01;
        const SEGMENTS: u32 = 8;

        let mut positions = Vec::new();

        let mut tex_coords = Vec::new();
        let mut normals = Vec::new();
        let mut tangents = Vec::new();

        let mut indices = Vec::new();

        for dir in [Vec3::X /* Vec3::Y, Vec3::Z */] {
            let tan = if dir.distance(Vec3::Y) < 0.001 {
                Vec3::Z
            } else {
                dir.cross(Vec3::Y)
            };

            let start_index = positions.len() as u32;

            let tip = dir * 1.0;
            positions.push(tip);
            tex_coords.push(vec2(0.0, 0.0));
            normals.push(dir);

            tangents.push(dir.cross(tan).normalize().extend(0.0));

            let bitan = dir.cross(tan);

            for theta in 0..SEGMENTS {
                let theta = theta as f32 * TAU / SEGMENTS as f32 + DEG_45;

                let x = theta.cos() * HEAD_RADIUS;
                let y = theta.sin() * HEAD_RADIUS;

                let point = tip - dir * HEAD_LENGTH + (tan * x + bitan * y);

                tex_coords.push(vec2(0.5 + x * 0.5, 0.5 + y * 0.5));
                tangents.push(dir.cross(tan).normalize().extend(0.0));

                normals.push(dir);
                positions.push(point);
            }

            // mantel
            indices.extend(
                (0..SEGMENTS)
                    .flat_map(|i| {
                        [
                            start_index,
                            start_index + i + 1,
                            start_index + (i + 1) % SEGMENTS + 1,
                        ]
                    })
                    .collect::<Vec<_>>(),
            );

            // bottom
            indices.extend(
                (0..SEGMENTS - 1)
                    .flat_map(|i| {
                        [
                            start_index + (i + 2) % SEGMENTS + 1,
                            start_index + i + 2,
                            start_index + 1,
                        ]
                    })
                    .collect::<Vec<_>>(),
            );

            let start_index = positions.len() as u32;

            // cylinder base
            for offset in [0.0, 1.0] {
                for theta in 0..SEGMENTS {
                    let theta = theta as f32 * TAU / SEGMENTS as f32 + DEG_45;

                    let x = theta.cos() * BASE_RADIUS;
                    let y = theta.sin() * BASE_RADIUS;

                    let point = dir * offset * (1.0 - HEAD_LENGTH) + (tan * x + bitan * y);

                    tex_coords.push(vec2(0.5 + x * 0.5, 0.5 + y * 0.5));
                    tangents.push(dir.cross(tan).normalize().extend(0.0));

                    normals.push(dir);
                    positions.push(point);
                }
            }

            // sides
            indices.extend(
                (0..SEGMENTS)
                    .flat_map(|i| {
                        [
                            start_index + i,
                            start_index + (i + 1) % SEGMENTS,
                            start_index + i + SEGMENTS,
                            start_index + (i + 1) % SEGMENTS,
                            start_index + (i + 1) % SEGMENTS + SEGMENTS,
                            start_index + i + SEGMENTS,
                        ]
                    })
                    .collect::<Vec<_>>(),
            );

            // bottom
            indices.extend(
                (0..SEGMENTS - 1)
                    .flat_map(|i| {
                        [
                            start_index + (i + 2) % SEGMENTS,
                            start_index + i + 1,
                            start_index,
                        ]
                    })
                    .collect::<Vec<_>>(),
            );
        }

        let mesh = MeshData::unskinned(indices, positions, tex_coords, normals)
            .with_attribute(TANGENT_ATTRIBUTE, tangents);
        Ok(assets.insert(mesh))
    }
}

pub fn main() -> anyhow::Result<()> {
    registry()
        .with(EnvFilter::from_default_env())
        .with(
            HierarchicalLayer::default()
                .with_indent_lines(true)
                .with_deferred_spans(true)
                .with_span_retrace(true),
        )
        .init();

    if let Err(err) = App::builder()
        .with_driver(WinitDriver::new(
            WindowAttributes::default()
                .with_inner_size(LogicalSize::new(1920, 1080))
                .with_title("Ivy"),
        ))
        .with_layer(EngineLayer::new())
        .with_layer(ProfilingLayer::new())
        .with_layer(GraphicsLayer::new(|world, assets, store, gpu, surface| {
            Ok(SurfacePbrRenderer::new(
                world,
                assets,
                store,
                gpu,
                surface,
                SurfacePbrPipelineDesc {
                    pbr_config: PbrRenderGraphConfig {
                        label: "basic".into(),
                        shadow_map_config: Some(Default::default()),
                        msaa: Some(Default::default()),
                        bloom: Some(Default::default()),
                        skybox: Some(SkyboxConfig {
                            hdri: Box::new(AssetPath::new(
                                "hdris/kloofendal_48d_partly_cloudy_puresky_2k.hdr",
                            )),
                            format: TextureFormat::Rgba16Float,
                        }),
                        hdr_format: Some(wgpu::TextureFormat::Rgba16Float),
                    },
                    ..Default::default()
                },
            ))
        }))
        .with_layer(InputLayer::new())
        .with_layer(UiLayer::new())
        .with_layer(
            ScheduledLayer::new(FixedTimeStep::new(0.02))
                .with_plugin(OrbitCameraPlugin)
                .with_plugin(StreamedUiPlugin)
                .with_plugin(ExamplePlugin)
                .with_plugin(
                    PhysicsPlugin::new()
                        .with_gravity(Vec3::ZERO)
                        .with_gizmos(GizmoSettings { rigidbody: true }),
                )
                .with_plugin(TransformUpdatePlugin),
        )
        .with_layer(UiUpdateLayer::new())
        .with_layer(ViewportCameraLayer::new(CameraSettings {
            environment_data: EnvironmentData::new(
                Srgb::new(0.2, 0.2, 0.3),
                0.001,
                if ENABLE_SKYBOX { 0.0 } else { 1.0 },
            ),
            fov: 1.0,
        }))
        .run()
    {
        tracing::error!("{err:?}");
        Err(err)
    } else {
        Ok(())
    }
}

pub struct LogicLayer {}

impl Default for LogicLayer {
    fn default() -> Self {
        Self::new()
    }
}

impl LogicLayer {
    pub fn new() -> Self {
        Self {}
    }

    fn setup_objects(&mut self, world: &mut World, assets: &AssetCache) -> anyhow::Result<()> {
        let mesh = MeshDesc::content(assets.load(&ArrowMesh));
        for axis in [Axis3D::X, Axis3D::Y, Axis3D::Z] {
            let color = match axis {
                Axis3D::X => Rgba([255, 0, 0, 255]),
                Axis3D::Y => Rgba([0, 255, 0, 255]),
                Axis3D::Z => Rgba([0, 0, 255, 255]),
            };

            let material = MaterialData::UnlitMaterial(
                PbrMaterialData::new().with_albedo(TextureData::Color(color)),
            );

            let mut builder = Entity::builder();
            builder
                .set(name(), format!("{axis:?}").into())
                .mount(
                    TransformBundle::default()
                        .with_rotation(Quat::from_rotation_arc(Vec3::X, axis.to_vec3())),
                )
                .mount(RenderObjectBundle::new(
                    mesh.clone(),
                    &[(transparent_pass(), material)],
                ))
                .spawn(world);
        }

        let material = MaterialData::PbrMaterial(
            PbrMaterialData::new()
                .with_roughness_factor(0.1)
                .with_metallic_factor(0.0)
                .with_albedo(TextureData::srgba(Srgba::new(1.0, 1.0, 1.0, 1.0))),
        );

        let cube_mesh = MeshDesc::Content(assets.load(&CubePrimitive));

        let simulate = true;

        let cube = |mut position: Vec3, rotation: Quat| {
            let mut velocity = Vec3::ZERO;
            if simulate {
                position.z = position.z.signum() * 4.0;

                velocity = Vec3::Z * -position.z.signum() * 1.0;
            }

            let mut builder = Entity::builder();
            builder
                .set(name(), "Cube".into())
                .mount(
                    TransformBundle::default()
                        .with_position(position + Vec3::Z)
                        .with_rotation(rotation),
                )
                .mount(RigidBodyBundle::dynamic().with_velocity(velocity))
                .mount(
                    ColliderBundle::new(SharedShape::cuboid(1.0, 1.0, 1.0))
                        .with_friction(0.7)
                        .with_restitution(0.1),
                )
                .mount(RenderObjectBundle::new(
                    cube_mesh.clone(),
                    &[(forward_pass(), material.clone())],
                ));

            builder
        };

        cube(vec3(0.2, 0.0, 0.99), Quat::IDENTITY)
            .attach(
                child_of,
                cube(
                    vec3(0.0, 0.0, -0.99),
                    Quat::from_scaled_axis(vec3(0.0, 0.0, 0.5)),
                ),
            )
            .spawn(world);

        cube(
            vec3(2.0, 0.0, -0.99),
            Quat::from_scaled_axis(vec3(0.0, 0.0, 0.5)),
        )
        .spawn(world);

        Ok(())
    }
}

struct ExamplePlugin;

impl Plugin for ExamplePlugin {
    fn install(
        &self,
        world: &mut World,
        assets: &AssetCache,
        _: &mut DynamicStore,
        _: &mut ivy_core::update_layer::ScheduleSetBuilder,
    ) -> anyhow::Result<()> {
        world.get(engine(), screen_state())?.open(MainUi);

        let mut layer = LogicLayer::new();
        layer.setup_objects(world, assets)?;

        Ok(())
    }
}

struct MainUi;

impl Screen for MainUi {
    fn create(self, scope: &mut violet::core::Scope<'_>, _: ScreenLifetimeToken) {
        maximized((
            label("Transforms").with_item_align(LayoutAlignment::new(Align::Center, Align::Start)),
            HierarchyPanel::new()
                .with_item_align(LayoutAlignment::new(Align::Start, Align::Center)),
        ))
        .mount(scope);
    }
}

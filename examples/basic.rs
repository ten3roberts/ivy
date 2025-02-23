use std::f32::consts::{PI, TAU};

use anyhow::Context;
use flax::{
    components::child_of, BoxedSystem, Component, Entity, FetchExt, Query, QueryBorrow, System,
    World,
};
use glam::{vec3, EulerRot, Mat4, Quat, Vec3};
use image::{DynamicImage, Rgba};
use itertools::{Either, Itertools};
use ivy_assets::{fs::AssetPath, loadable::Load, Asset, AssetCache};
use ivy_core::{
    app::PostInitEvent,
    gizmos,
    layer::events::EventRegisterContext,
    palette::{Srgb, WithAlpha},
    profiling::ProfilingLayer,
    update_layer::{FixedTimeStep, Plugin, ScheduleSetBuilder, ScheduledLayer},
    App, EngineLayer, EntityBuilderExt, Layer,
};
use ivy_engine::{
    async_commandbuffer, elapsed_time, engine, rotation, world_transform, RigidBodyBundle,
    TransformBundle,
};
use ivy_game::{
    orbit_camera::OrbitCameraPlugin,
    ray_picker::RayPickingPlugin,
    viewport_camera::{CameraSettings, ViewportCameraLayer},
};
use ivy_gltf::{
    animation::{
        player::{AnimationPlayer, Animator},
        plugin::AnimationPlugin,
    },
    Document,
};
use ivy_graphics::texture::{ColorChannel, MetallicRoughnessProcessor, TextureData, TextureDesc};
use ivy_input::layer::InputLayer;
use ivy_physics::{ColliderBundle, GizmoSettings, PhysicsPlugin};
use ivy_postprocessing::preconfigured::{
    pbr::{PbrRenderGraphConfig, SkyboxConfig},
    SurfacePbrPipelineDesc, SurfacePbrRenderer,
};
use ivy_scene::{GltfNodeExt, NodeMountOptions};
use ivy_wgpu::{
    components::{forward_pass, light_kind, light_params, shadow_pass, transparent_pass},
    driver::WinitDriver,
    layer::GraphicsLayer,
    light::{LightBundle, LightKind, LightParams},
    material_desc::{
        MaterialData, MaterialDesc, PbrEmissiveMaterialData, PbrMaterialData, PbrMaterialDesc,
    },
    mesh_desc::MeshDesc,
    primitives::{generate_plane, UvSpherePrimitive},
    renderer::{EnvironmentData, RenderObjectBundle},
};
use rapier3d::prelude::SharedShape;
use tracing::Instrument;
use tracing_subscriber::{layer::SubscriberExt, registry, util::SubscriberInitExt, EnvFilter};
use tracing_tree::HierarchicalLayer;
use violet::palette::{rgb::Rgb, Hsl, IntoColor};
use wgpu::TextureFormat;
use winit::{dpi::LogicalSize, window::WindowAttributes};

const ENABLE_SKYBOX: bool = true;

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
                        skybox: Some(SkyboxConfig {
                            hdri: Box::new(AssetPath::new(
                                "hdris/kloofendal_48d_partly_cloudy_puresky_2k.hdr",
                            )),
                            format: TextureFormat::Rgba16Float,
                        }),
                        ..Default::default()
                    },
                    ..Default::default()
                },
            ))
        }))
        .with_layer(InputLayer::new())
        .with_layer(LogicLayer::new())
        .with_layer(
            ScheduledLayer::new(FixedTimeStep::new(0.02))
                .with_plugin(OrbitCameraPlugin)
                .with_plugin(GizmosPlugin)
                .with_plugin(AnimationPlugin)
                .with_plugin(
                    PhysicsPlugin::new()
                        .with_gravity(-Vec3::Y * 9.81)
                        .with_gizmos(GizmoSettings { rigidbody: true }),
                )
                .with_plugin(RotateSpotlightPlugin)
                .with_plugin(RayPickingPlugin),
        )
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

pub struct GizmosPlugin;

impl Plugin for GizmosPlugin {
    fn install(
        &self,
        _: &mut World,
        _: &AssetCache,
        schedules: &mut ScheduleSetBuilder,
    ) -> anyhow::Result<()> {
        schedules
            .per_tick_mut()
            .with_system(point_light_gizmo_system());

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

    fn setup_assets(&mut self, world: &mut World, assets: &AssetCache) -> anyhow::Result<()> {
        let cmd = world.get(engine(), async_commandbuffer()).unwrap().clone();
        let assets = assets.clone();

        const DENSITY: f32 = 10.0;
        const FRICTION: f32 = 0.5;
        const RESTITUTION: f32 = 0.1;

        let future = async move {
            let plane_mesh = MeshDesc::content(assets.insert(generate_plane(8.0, Vec3::Y)));

            let texture_group = "textures/BaseCollection/Sand";
            let albedo = AssetPath::new(format!("{texture_group}/albedo.png"));

            let normal = AssetPath::new(format!("{texture_group}/normal.png"));

            let roughness = AssetPath::new(format!("{texture_group}/roughness.png"));

            let ao = AssetPath::new(format!("{texture_group}/ao.png"));

            let displacement = AssetPath::new(format!("{texture_group}/displacement.png"));

            let plane_material = MaterialDesc::PbrMaterial(
                PbrMaterialDesc::new()
                    .with_metallic_factor(0.0)
                    .with_albedo(TextureDesc::Path(albedo))
                    .with_normal(TextureDesc::Path(normal))
                    .with_metallic_roughness(TextureDesc::Path(roughness).process(
                        MetallicRoughnessProcessor::new(
                            Either::Right(0),
                            Either::Left(ColorChannel::Red),
                        ),
                    ))
                    .with_ambient_occlusion(TextureDesc::Path(ao))
                    .with_displacement(TextureDesc::Path(displacement)),
            )
            .load(&assets)
            .await?;

            cmd.lock().spawn(
                Entity::builder()
                    .mount(TransformBundle::new(
                        Vec3::ZERO,
                        Quat::IDENTITY,
                        Vec3::ONE * 2.0,
                    ))
                    .mount(RenderObjectBundle::new(
                        plane_mesh.clone(),
                        &[
                            (forward_pass(), plane_material),
                            (shadow_pass(), MaterialData::ShadowMaterial),
                        ],
                    ))
                    .mount(RigidBodyBundle::fixed())
                    .mount(
                        ColliderBundle::new(SharedShape::cuboid(16.0, 0.01, 16.0))
                            .with_density(DENSITY)
                            .with_restitution(RESTITUTION)
                            .with_friction(FRICTION),
                    ),
            );

            let sphere_mesh = MeshDesc::content(assets.load(&UvSpherePrimitive::default()));

            let unlit_material = MaterialData::PbrMaterial(
                PbrMaterialData::new()
                    .with_metallic_factor(1.0)
                    .with_roughness_factor(0.1)
                    .with_albedo(TextureData::Color(Rgba([255, 255, 255, 128]))),
            );
            Entity::builder()
                .mount(TransformBundle::default().with_position(vec3(5.0, 2.0, 0.0)))
                .mount(RenderObjectBundle::new(
                    sphere_mesh.clone(),
                    &[
                        (transparent_pass(), unlit_material.clone()),
                        (shadow_pass(), MaterialData::ShadowMaterial),
                    ],
                ))
                .spawn_into(&mut cmd.lock());

            let albedo = assets
                .from_path("textures/BaseCollection/Porcelein/albedo.png")
                .await?;

            let normal = assets
                .from_path("textures/BaseCollection/Porcelein/normal.png")
                .await?;

            let roughness: Asset<DynamicImage> = assets
                .from_path("textures/BaseCollection/Porcelein/roughness.png")
                .await?;

            let emissive_material = MaterialData::EmissiveMaterial(PbrEmissiveMaterialData::new(
                PbrMaterialData::new()
                    .with_albedo(TextureData::Content(albedo))
                    .with_normal(TextureData::Content(normal))
                    .with_metallic_roughness(TextureData::Content(roughness.clone()).process(
                        MetallicRoughnessProcessor::new(
                            Either::Right(0),
                            Either::Left(ColorChannel::Red),
                        ),
                    )),
                TextureData::Content(roughness),
                50.0,
            ));
            Entity::builder()
                .mount(
                    TransformBundle::default()
                        .with_position(vec3(-5.0, 2.0, 0.0))
                        .with_scale(Vec3::splat(0.25)),
                )
                .mount(RenderObjectBundle::new(
                    sphere_mesh.clone(),
                    &[
                        (forward_pass(), emissive_material.clone()),
                        (shadow_pass(), MaterialData::ShadowMaterial),
                    ],
                ))
                .mount(LightBundle {
                    params: LightParams::new(Srgb::new(1.0, 1.0, 1.0), 2.0),
                    kind: LightKind::Point,
                    cast_shadow: false,
                })
                .spawn_into(&mut cmd.lock());

            let roughness_count = 16;
            for i in 0..roughness_count {
                let roughness = i as f32 / (roughness_count - 1) as f32;
                for j in 0..2 {
                    let metallic = j as f32;

                    let plastic_material = MaterialData::PbrMaterial(
                        PbrMaterialData::new()
                            .with_metallic_factor(metallic)
                            .with_roughness_factor(roughness),
                    );

                    let phi = (i as f32 / roughness_count as f32) * TAU
                        + j as f32 * PI / roughness_count as f32;

                    let radius = 8.0 + j as f32 * 3.0;
                    cmd.lock().spawn(
                        Entity::builder()
                            .mount(TransformBundle::default().with_position(vec3(
                                phi.cos() * radius,
                                1.0,
                                phi.sin() * radius,
                            )))
                            .mount(RenderObjectBundle::new(
                                sphere_mesh.clone(),
                                &[
                                    (forward_pass(), plastic_material.clone()),
                                    (shadow_pass(), MaterialData::ShadowMaterial),
                                ],
                            )),
                    );
                }
            }

            let document: Asset<Document> = assets.from_path("models/Gears.glb").await.unwrap();
            tracing::info!(
                "{:?}",
                document
                    .nodes()
                    .map(|v| v.name().map(|v| v.to_string()))
                    .collect_vec()
            );
            let node = document
                .find_node("Gears")
                .context("Missing document node")
                .unwrap();

            let skin = node.skin().unwrap();
            let animation = skin.animations()[0].clone();

            let mut animator = Animator::new();
            let mut player = AnimationPlayer::new(animation);
            player.set_looping(true);
            player.set_speed(0.2);
            animator.start_animation(player);

            node.mount(
                &mut Entity::builder(),
                &NodeMountOptions {
                    skip_empty_children: true,
                    material_overrides: &Default::default(),
                },
            )
            .mount(TransformBundle::new(
                vec3(0.0, 0.5, 0.0),
                Quat::IDENTITY,
                Vec3::ONE,
            ))
            .set(ivy_gltf::components::animator(), animator)
            .spawn_into(&mut cmd.lock());

            anyhow::Ok(())
        };

        async_std::task::spawn(future.instrument(tracing::debug_span!("load_assets")));

        Ok(())
    }
}

struct RotateSpotlightPlugin;

impl Plugin for RotateSpotlightPlugin {
    fn install(
        &self,
        world: &mut World,
        _: &AssetCache,
        schedules: &mut ScheduleSetBuilder,
    ) -> anyhow::Result<()> {
        flax::component! {
            rotate_light: Quat,
        }

        let count = 3;
        let parent = Entity::builder()
            .mount(TransformBundle::default().with_position(vec3(0.0, 4.0, 0.0)))
            .set(rotate_light(), Quat::IDENTITY)
            .spawn(world);

        Entity::builder()
            .mount(
                TransformBundle::default()
                    .with_position(vec3(0.0, 5.0, -1.0))
                    .with_rotation(Quat::from_euler(EulerRot::YXZ, 0.0, -PI / 2.0 - 0.5, 0.0)),
            )
            .mount(LightBundle {
                params: LightParams::new(Rgb::new(1.0, 1.0, 1.0), 25.0)
                    .with_angular_cutoffs(0.4, 0.5),
                kind: LightKind::Spotlight,
                cast_shadow: true,
            })
            .set(child_of(parent), ())
            .spawn(world);

        for i in 0..count {
            let phi = (i as f32 / count as f32) * TAU;

            let radius = 1.0;
            Entity::builder()
                .mount(
                    TransformBundle::default()
                        .with_position(vec3(phi.sin() * radius, 0.0, phi.cos() * radius))
                        .with_rotation(Quat::from_euler(EulerRot::YXZ, phi, PI + 0.5, 0.0)),
                )
                .mount(LightBundle {
                    params: LightParams::new(
                        Hsl::new(phi * 180.0 / PI, 1.0, 0.5).into_color(),
                        25.0,
                    )
                    .with_angular_cutoffs(0.4, 0.5),
                    kind: LightKind::Spotlight,
                    cast_shadow: true,
                })
                .set(child_of(parent), ())
                .spawn(world);
        }

        schedules.fixed_mut().with_system(
            System::builder()
                .with_query(Query::new((
                    rotate_light(),
                    rotation().as_mut(),
                    elapsed_time().source(engine()),
                )))
                .for_each(move |(&base_rotation, rotation, &t)| {
                    *rotation =
                        Quat::from_axis_angle(Vec3::Y, t.as_secs_f32() * 0.1) * base_rotation;
                }),
        );

        Ok(())
    }
}

impl Layer for LogicLayer {
    fn register(
        &mut self,
        _: &mut World,
        _: &AssetCache,
        mut events: EventRegisterContext<Self>,
    ) -> anyhow::Result<()> {
        events.subscribe(|this, ctx, _: &PostInitEvent| this.setup_assets(ctx.world, ctx.assets));

        Ok(())
    }
}

fn point_light_gizmo_system() -> BoxedSystem {
    System::builder()
        .with_query(Query::new(ivy_core::components::gizmos().source(engine())))
        .with_query(Query::new((
            world_transform(),
            light_params(),
            light_kind(),
        )))
        .build(
            |mut gizmos: QueryBorrow<flax::fetch::Source<Component<gizmos::Gizmos>, Entity>>,
             mut query: QueryBorrow<(
                Component<Mat4>,
                Component<LightParams>,
                Component<LightKind>,
            )>| {
                let mut gizmos = gizmos
                    .first()
                    .unwrap()
                    .begin_section("point_light_gizmo_system");

                query
                    .iter()
                    .for_each(|(transform, light, kind)| match kind {
                        LightKind::Point => gizmos.draw(gizmos::Sphere::new(
                            transform.transform_point3(Vec3::ZERO),
                            0.1,
                            light.color.with_alpha(1.0),
                        )),
                        LightKind::Directional | LightKind::Spotlight => {
                            let pos = transform.transform_point3(Vec3::ZERO);
                            let dir = transform.transform_vector3(Vec3::FORWARD);

                            gizmos.draw(gizmos::Sphere::new(pos, 0.1, light.color.with_alpha(1.0)));

                            gizmos.draw(gizmos::Line::new(
                                pos,
                                dir,
                                0.02,
                                light.color.with_alpha(1.0),
                            ))
                        }
                    });
            },
        )
        .boxed()
}

use std::f32::consts::{PI, TAU};

use flax::{
    components::child_of, BoxedSystem, Component, Entity, FetchExt, Query, QueryBorrow, System,
    World,
};
use glam::{vec3, EulerRot, Mat4, Quat, Vec3};
use image::{DynamicImage, Rgba};
use ivy_assets::loadable::Loadable;
use ivy_assets::{stored::DynamicStore, Asset, AssetCache, AssetPath, AsyncAssetExt};
use ivy_core::{
    app::PostInitEvent,
    gizmos,
    layer::events::EventRegisterContext,
    math::Vec3Ext,
    palette::{Srgb, WithAlpha},
    profiling::ProfilingLayer,
    transforms::TransformUpdatePlugin,
    update_layer::{FixedTimeStep, Plugin, PluginLayer, ScheduleSetBuilder},
    App, AsyncCommandBuffer, EngineLayer, EntityBuilderExt, Layer,
};
use ivy_engine::{
    async_commandbuffer, elapsed_time, engine, rotation, world_transform, RigidBodyBundle,
    TransformBundle,
};
use ivy_game::{
    debug::AssetTimelinesWidget, orbit_camera::OrbitCameraPlugin,
    viewport_camera::CameraViewportPlugin,
};
use ivy_gltf::{
    animation::{
        player::{AnimationPlayer, Animator},
        plugin::AnimationPlugin,
        AnimationDesc,
    },
    Document,
};
use ivy_graphics::texture::{TextureData, TextureDesc};
use ivy_input::layer::InputLayer;
use ivy_physics::{ColliderBundle, GizmoSettings, PhysicsPlugin};
use ivy_postprocessing::preconfigured::{
    pbr::{PbrRenderGraphConfig, SkyboxConfig},
    SurfacePbrPipelineDesc, SurfacePbrRenderer,
};
use ivy_scene::{GltfNodeExt, NodeMountOptions};
use ivy_ui::{
    layer::{UiLayer, UiUpdateLayer},
    screens::{screen_state, Screen},
};
use ivy_wgpu::material::{EffectPass, Material, MaterialBundle};
use ivy_wgpu::renderer::MeshBundle;
use ivy_wgpu::{
    components::{forward_pass, light_kind, light_params, shadow_pass, transparent_pass},
    driver::WinitDriver,
    effect_desc::{
        PbrEmissiveRenderEffectDesc, PbrRenderEffect, PbrRenderEffectDesc, RenderEffect,
        RenderEffectDesc,
    },
    layer::GraphicsLayer,
    light::{LightBundle, LightKind, LightParams},
    mesh_desc::MeshDesc,
    primitives::{generate_plane, UvSpherePrimitive},
    renderer::RenderObjectBundle,
};
use rapier3d::prelude::SharedShape;
use tracing_subscriber::{layer::SubscriberExt, registry, util::SubscriberInitExt, EnvFilter};
use tracing_tree::HierarchicalLayer;
use violet::{
    core::{
        style::SizeExt,
        unit::Unit,
        widget::{card, maximized},
        Widget,
    },
    palette::{rgb::Rgb, Hsl, IntoColor},
};
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

    let ui_input_layer = UiLayer::new();
    let ui_layer = UiUpdateLayer::new();

    if let Err(err) = App::builder()
        .with_driver(WinitDriver::new(
            WindowAttributes::default()
                .with_inner_size(LogicalSize::new(1920, 1080))
                .with_title("Ivy"),
        ))
        .with_layer(EngineLayer::new())
        .with_layer(ProfilingLayer::new())
        .with_layer(GraphicsLayer::new(
            move |world, assets, store, gpu, surface| {
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
                    },
                ))
            },
        ))
        .with_layer(ui_input_layer)
        .with_layer(InputLayer::new())
        .with_layer(LogicLayer::new())
        .with_layer(
            PluginLayer::new(FixedTimeStep::new(0.02))
                .with_plugin(GameUiPlugin)
                .with_plugin(OrbitCameraPlugin)
                .with_plugin(GizmosPlugin)
                .with_plugin(AnimationPlugin)
                .with_plugin(
                    PhysicsPlugin::new()
                        .with_gravity(-Vec3::Y * 9.81)
                        .with_gizmos(GizmoSettings { rigidbody: true }),
                )
                .with_plugin(RotateSpotlightPlugin)
                .with_plugin(TransformUpdatePlugin),
        )
        // .with_layer(CameraViewportPlugin::new(CameraSettings {
        //     environment_data: EnvironmentData::new(
        //         Srgb::new(0.2, 0.2, 0.3),
        //         0.001,
        //         if ENABLE_SKYBOX { 0.0 } else { 1.0 },
        //     ),
        //     fov: 1.0,
        // }))
        .with_layer(ui_layer)
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
        _: &mut DynamicStore,
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

    fn setup_assets(&self, world: &mut World, assets: &AssetCache) -> anyhow::Result<()> {
        let cmd = world.get(engine(), async_commandbuffer()).unwrap().clone();
        let assets = assets.clone();

        const DENSITY: f32 = 10.0;
        const FRICTION: f32 = 0.5;
        const RESTITUTION: f32 = 0.1;

        async fn load_objects(assets: AssetCache, cmd: AsyncCommandBuffer) -> anyhow::Result<()> {
            let plane_mesh = MeshDesc::content(assets.insert(generate_plane(8.0, Vec3::Y)));

            let texture_group = "textures/BaseCollection/Sand";
            let albedo = AssetPath::new(format!("{texture_group}/albedo.png"));
            let normal = AssetPath::new(format!("{texture_group}/normal.png"));

            let roughness: AssetPath<DynamicImage> =
                AssetPath::new(format!("{texture_group}/roughness.png"));

            let ao = AssetPath::new(format!("{texture_group}/ao.png"));

            let displacement = AssetPath::new(format!("{texture_group}/displacement.png"));

            use ivy_assets::loadable::Loadable;

            let plane_material = RenderEffectDesc::Pbr(
                PbrRenderEffectDesc::new()
                    .with_metallic_factor(0.0)
                    .with_albedo(TextureDesc::Path(albedo))
                    .with_normal(TextureDesc::Path(normal))
                    .with_metallic_roughness(TextureDesc::Path(roughness))
                    .with_ambient_occlusion(TextureDesc::Path(ao))
                    .with_displacement(TextureDesc::Path(displacement)),
            )
            .load(&assets)
            .await?;

            let emissive_material = RenderEffectDesc::Emissive(PbrEmissiveRenderEffectDesc::new(
                PbrRenderEffectDesc::new().with_albedo(TextureDesc::Color(255, 255, 255, 50)),
                TextureDesc::Color(255, 255, 255, 255),
                20.0,
            ))
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
                            (shadow_pass(), RenderEffect::OpaqueShadow),
                        ],
                    ))
                    .mount(RigidBodyBundle::fixed())
                    .mount(
                        ColliderBundle::new(SharedShape::cuboid(16.0, 0.1, 16.0))
                            .with_density(DENSITY)
                            .with_restitution(RESTITUTION)
                            .with_friction(FRICTION),
                    ),
            );

            let sphere_mesh = MeshDesc::content(assets.load(&UvSpherePrimitive::default()));

            let unlit_material = RenderEffect::Pbr(
                PbrRenderEffect::new()
                    .with_metallic_factor(0.0)
                    .with_roughness_factor(0.2)
                    .with_albedo(TextureData::Color(Rgba([255, 255, 255, 128]))),
            );
            Entity::builder()
                .mount(TransformBundle::default().with_position(vec3(5.0, 2.0, 0.0)))
                .mount(RenderObjectBundle::new(
                    sphere_mesh.clone(),
                    &[
                        (transparent_pass(), unlit_material.clone()),
                        (shadow_pass(), RenderEffect::OpaqueShadow),
                    ],
                ))
                .spawn_into(&mut cmd.lock());

            Entity::builder()
                .mount(
                    TransformBundle::default()
                        .with_position(vec3(-5.0, 2.0, 0.0))
                        .with_scale(Vec3::splat(0.25)),
                )
                .mount(RenderObjectBundle::new(
                    sphere_mesh.clone(),
                    &[(transparent_pass(), emissive_material.clone())],
                ))
                .mount(LightBundle {
                    params: LightParams::new(Srgb::new(1.0, 1.0, 1.0), 5.0),
                    kind: LightKind::Point,
                    cast_shadow: false,
                })
                .spawn_into(&mut cmd.lock());

            let roughness_count = 16;
            for i in 0..roughness_count {
                let roughness = i as f32 / (roughness_count - 1) as f32;
                for j in 0..2 {
                    let metallic = j as f32;

                    let phi = (i as f32 / roughness_count as f32) * TAU
                        + j as f32 * PI / roughness_count as f32;

                    // one-off material
                    let material = assets.insert(
                        Material::new()
                            .with_effect(
                                EffectPass::Forward,
                                RenderEffect::Pbr(
                                    PbrRenderEffect::new()
                                        .with_metallic_factor(metallic)
                                        .with_roughness_factor(roughness),
                                ),
                            )
                            .with_effect(EffectPass::Shadow, RenderEffect::OpaqueShadow),
                    );

                    let radius = 8.0 + j as f32 * 3.0;
                    cmd.lock().spawn(
                        Entity::builder()
                            .mount(TransformBundle::default().with_position(vec3(
                                phi.cos() * radius,
                                1.0,
                                phi.sin() * radius,
                            )))
                            .mount(MeshBundle::new(sphere_mesh.clone()))
                            .mount(MaterialBundle::new(material)),
                    );
                }
            }

            anyhow::Ok(())
        }

        async fn load_gears(assets: AssetCache, cmd: AsyncCommandBuffer) -> anyhow::Result<()> {
            let document: Asset<Document> = AssetPath::new("models/Gears.glb")
                .load_async(&assets)
                .await
                .unwrap();

            for node in document.nodes() {
                let animation = assets
                    .try_load_async(&AnimationDesc {
                        document: "models/Gears.glb".into(),
                        animation: "ArmatureAction.001".into(),
                    })
                    .await?;

                let mut player = AnimationPlayer::new(animation);
                player.set_looping(true);
                player.set_speed(0.5);

                let mut animator = Animator::new();
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
            }

            anyhow::Ok(())
        }

        async_std::task::spawn(load_objects(assets.clone(), cmd.clone()));
        async_std::task::spawn(load_gears(assets, cmd));

        Ok(())
    }
}

struct RotateSpotlightPlugin;

impl Plugin for RotateSpotlightPlugin {
    fn install(
        &self,
        world: &mut World,
        _: &AssetCache,
        _: &mut DynamicStore,
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

struct GameUiPlugin;

impl Plugin for GameUiPlugin {
    fn install(
        &self,
        world: &mut World,
        assets: &AssetCache,
        _: &mut DynamicStore,
        _: &mut ScheduleSetBuilder,
    ) -> anyhow::Result<()> {
        world.get(engine(), screen_state())?.open(MainUI {
            assets: assets.clone(),
        });

        Ok(())
    }
}

struct MainUI {
    assets: AssetCache,
}

impl Screen for MainUI {
    fn create(self, scope: &mut violet::core::Scope<'_>, _: ivy_ui::screens::ScreenLifetimeToken) {
        maximized(
            card(AssetTimelinesWidget::new(self.assets)).with_max_size(Unit::rel2(0.25, 1.0)),
        )
        .mount(scope);
    }
}

impl Layer for LogicLayer {
    fn register(
        &mut self,
        _: &mut World,
        _: &AssetCache,
        _: &mut DynamicStore,
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
                        LightKind::Point => gizmos.draw(gizmos::SphereGizmo::new(
                            transform.transform_point3(Vec3::ZERO),
                            0.1,
                            light.color.with_alpha(1.0),
                        )),
                        LightKind::Directional | LightKind::Spotlight => {
                            let pos = transform.transform_point3(Vec3::ZERO);
                            let dir = transform.transform_vector3(Vec3::FORWARD);

                            gizmos.draw(gizmos::SphereGizmo::new(
                                pos,
                                0.1,
                                light.color.with_alpha(1.0),
                            ));

                            gizmos.draw(gizmos::LineGizmo::new(
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

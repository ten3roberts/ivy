use std::sync::Arc;

use anyhow::Context;
use flax::{
    component, BoxedSystem, Component, Entity, EntityBuilder, FetchExt, Mutable, Query,
    QueryBorrow, ScheduleBuilder, System, World,
};
use glam::{vec3, EulerRot, Mat4, Quat, Vec2, Vec3};
use ivy_assets::{Asset, AssetCache};
use ivy_collision::components::collider;
use ivy_core::{
    app::InitEvent,
    gizmos,
    layer::events::EventRegisterContext,
    palette::{Srgb, WithAlpha},
    profiling::ProfilingLayer,
    update_layer::{FixedTimeStep, PerTick, Plugin, ScheduledLayer},
    App, EngineLayer, EntityBuilderExt, Layer, DEG_180,
};
use ivy_engine::{
    async_commandbuffer, delta_time, engine, gravity_influence, is_static, main_camera, velocity,
    world_transform, Collider, RigidBodyBundle, TransformBundle,
};
use ivy_game::free_camera::{
    camera_movement, camera_speed, euler_rotation, pan_active, rotation_input, setup_camera,
    CameraInputPlugin,
};
use ivy_gltf::{animation::player::Animator, components::animator, Document};
use ivy_graphics::texture::TextureDesc;
use ivy_input::{
    components::input_state,
    layer::InputLayer,
    types::{Key, NamedKey},
    Action, BindingExt, CursorMovement, InputState, KeyBinding, MouseButtonBinding,
};
use ivy_physics::{GizmoSettings, PhysicsPlugin};
use ivy_postprocessing::preconfigured::{SurfacePbrPipeline, SurfacePbrPipelineDesc};
use ivy_scene::{GltfNodeExt, NodeMountOptions};
use ivy_wgpu::{
    components::{
        cast_shadow, environment_data, light_data, light_kind, projection_matrix, shadow_pass,
    },
    driver::WinitDriver,
    events::ResizedEvent,
    layer::GraphicsLayer,
    light::{LightData, LightKind},
    material_desc::{MaterialData, MaterialDesc},
    mesh_desc::MeshDesc,
    primitives::{generate_plane, CubePrimitive, UvSpherePrimitive},
    renderer::{EnvironmentData, RenderObjectBundle},
    rendergraph::{self, ExternalResources, RenderGraph},
    shader_library::{ModuleDesc, ShaderLibrary},
    shaders::{PbrShaderDesc, ShadowShaderDesc},
    Gpu,
};
use ivy_wgpu_types::{PhysicalSize, Surface};
use tracing::Instrument;
use tracing_subscriber::{layer::SubscriberExt, registry, util::SubscriberInitExt, EnvFilter};
use tracing_tree::HierarchicalLayer;
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
        .with_layer(GraphicsLayer::new(|world, assets, gpu, surface| {
            Ok(SurfacePbrPipeline::new(
                world,
                assets,
                gpu,
                surface,
                SurfacePbrPipelineDesc {
                    hdri: Some(Box::new("hdris/lauter_waterfall_4k.hdr")),
                },
            ))
        }))
        .with_layer(InputLayer::new())
        .with_layer(LogicLayer::new())
        .with_layer(
            ScheduledLayer::new(PerTick)
                .with_plugin(CameraInputPlugin)
                .with_plugin(GizmosPlugin),
        )
        .with_layer(
            ScheduledLayer::new(FixedTimeStep::new(0.02)).with_plugin(
                PhysicsPlugin::new()
                    .with_gravity(-Vec3::Y * 9.81)
                    .with_gizmos(GizmoSettings {
                        bvh_tree: false,
                        island_graph: false,
                        rigidbody: true,
                        contacts: true,
                    }),
            ),
        )
        .run()
    {
        tracing::error!("{err:?}");
        Err(err)
    } else {
        Ok(())
    }
}

pub struct AnimationPlugin;

impl Plugin<PerTick> for AnimationPlugin {
    fn install(
        &self,
        _: &mut World,
        _: &AssetCache,
        schedule: &mut ScheduleBuilder,
        _: &PerTick,
    ) -> anyhow::Result<()> {
        schedule.with_system(animate_system());
        Ok(())
    }
}

pub struct GizmosPlugin;

impl Plugin<PerTick> for GizmosPlugin {
    fn install(
        &self,
        _: &mut World,
        _: &AssetCache,
        schedule: &mut ScheduleBuilder,
        _: &PerTick,
    ) -> anyhow::Result<()> {
        schedule.with_system(point_light_gizmo_system());

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

        async_std::task::spawn({
            let cmd = cmd.clone();
            let assets = assets.clone();
            async move {
                let shader = assets.load(&PbrShaderDesc);
                let sphere_mesh = MeshDesc::content(assets.load(&UvSpherePrimitive::default()));
                let materials: Asset<Document> = assets.load_async("textures/materials.glb").await;

                {
                    let mut cmd = cmd.lock();

                    for (i, material) in materials.materials().enumerate() {
                        cmd.spawn(
                            Entity::builder()
                                .mount(TransformBundle::new(
                                    vec3(0.0 + i as f32 * 2.0, 1.0, 12.0),
                                    Quat::IDENTITY,
                                    Vec3::ONE * 0.5,
                                ))
                                .mount(RenderObjectBundle {
                                    mesh: sphere_mesh.clone(),
                                    material: material.into(),
                                    shader: shader.clone(),
                                }),
                        );
                    }
                }
            }
        });

        async_std::task::spawn(
            async move {
                let shader = assets.load(&PbrShaderDesc);

                let plane_mesh = MeshDesc::content(assets.insert(generate_plane(8.0, Vec3::Y)));

                let texture_group = "textures/BaseCollection/ConcreteTiles/Concrete007_2K-PNG";
                let plane_material = MaterialDesc::content(
                    MaterialData::new()
                        .with_metallic_factor(0.0)
                        .with_albedo(TextureDesc::path(format!("{texture_group}_Color.png")))
                        .with_normal(TextureDesc::path(format!("{texture_group}_NormalGL.png")))
                        .with_metallic_roughness(TextureDesc::path(format!(
                            "{texture_group}_Roughness.png"
                        )))
                        .with_ambient_occlusion(TextureDesc::path(format!(
                            "{texture_group}_AmbientOcclusion.png"
                        )))
                        .with_displacement(TextureDesc::path(format!(
                            "{texture_group}_Displacement.png"
                        ))),
                );

                cmd.lock().spawn(
                    Entity::builder()
                        .mount(TransformBundle::new(
                            Vec3::ZERO,
                            Quat::IDENTITY,
                            Vec3::ONE * 2.0,
                        ))
                        .mount(RenderObjectBundle {
                            mesh: plane_mesh.clone(),
                            material: plane_material.clone(),
                            shader: shader.clone(),
                        })
                        .mount(
                            RigidBodyBundle::default()
                                .with_restitution(1.0)
                                .with_friction(0.8),
                        )
                        .set(is_static(), ())
                        .set(
                            collider(),
                            Collider::cube_from_center(Vec3::Y * -0.05, vec3(8.0, 0.1, 8.0)),
                        )
                        .set(shadow_pass(), assets.load(&ShadowShaderDesc)),
                );

                let sphere_mesh = MeshDesc::content(assets.load(&UvSpherePrimitive::default()));
                let cube_mesh = MeshDesc::content(assets.load(&CubePrimitive));

                for i in 0..8 {
                    let roughness = i as f32 / (7) as f32;
                    for j in 0..2 {
                        let metallic = j as f32;

                        let plastic_material = MaterialDesc::content(
                            MaterialData::new()
                                .with_metallic_factor(metallic)
                                .with_roughness_factor(roughness),
                        );

                        cmd.lock().spawn(
                            Entity::builder()
                                .mount(TransformBundle::new(
                                    vec3(0.0 + i as f32 * 2.0, 1.0, 5.0 + 4.0 * j as f32),
                                    Quat::IDENTITY,
                                    Vec3::ONE * 0.5,
                                ))
                                .mount(RenderObjectBundle {
                                    mesh: sphere_mesh.clone(),
                                    material: plastic_material.clone(),
                                    shader: shader.clone(),
                                })
                                .mount(
                                    RigidBodyBundle::default()
                                        .with_mass(10.0)
                                        .with_angular_mass(50.0)
                                        .with_friction(0.5)
                                        .with_restitution(0.4),
                                )
                                .set(gravity_influence(), 1.0)
                                .set(collider(), Collider::sphere(1.0))
                                .set(shadow_pass(), assets.load(&ShadowShaderDesc)),
                        );
                    }
                }

                for i in 0..8 {
                    let roughness = i as f32 / (7) as f32;
                    for j in 0..2 {
                        let metallic = j as f32;

                        let plastic_material = MaterialDesc::content(
                            MaterialData::new()
                                .with_metallic_factor(metallic)
                                .with_roughness_factor(roughness),
                        );

                        cmd.lock().spawn(
                            Entity::builder()
                                .mount(TransformBundle::new(
                                    vec3(0.0 + i as f32 * 2.0, 2.0, 5.1 + 4.0 * j as f32),
                                    Quat::IDENTITY,
                                    Vec3::ONE,
                                ))
                                .mount(RenderObjectBundle {
                                    mesh: cube_mesh.clone(),
                                    material: plastic_material.clone(),
                                    shader: shader.clone(),
                                })
                                .mount(
                                    RigidBodyBundle::default()
                                        .with_mass(10.0)
                                        .with_angular_mass(50.0)
                                        .with_friction(0.8)
                                        .with_restitution(0.0),
                                )
                                .set(gravity_influence(), 1.0)
                                .set(
                                    collider(),
                                    Collider::cube_from_center(Vec3::ZERO, Vec3::ONE),
                                )
                                .set(shadow_pass(), assets.load(&ShadowShaderDesc)),
                        );
                    }
                }

                tracing::info!("loading spine");
                let document: Asset<Document> = assets.load_async("models/spine.glb").await;
                let node = document
                    .find_node("Cube")
                    .context("Missing document node")
                    .unwrap();

                let skin = node.skin().unwrap();
                let animation = skin.animations()[0].clone();

                let root: EntityBuilder = node
                    .mount(
                        &assets,
                        &mut Entity::builder(),
                        NodeMountOptions { cast_shadow: true },
                    )
                    .mount(TransformBundle::new(
                        vec3(0.0, 0.0, 2.0),
                        Quat::IDENTITY,
                        Vec3::ONE,
                    ))
                    .set(animator(), Animator::new(animation))
                    .into();

                cmd.lock().spawn(root);

                let document: Asset<Document> = assets.load_async("models/crate.glb").await;
                let node = document
                    .find_node("Cube")
                    .context("Missing document node")
                    .unwrap();

                let root: EntityBuilder = node
                    .mount(
                        &assets,
                        &mut Entity::builder(),
                        NodeMountOptions { cast_shadow: true },
                    )
                    .mount(TransformBundle::new(
                        vec3(0.0, 1.0, -2.0),
                        Quat::IDENTITY,
                        Vec3::ONE,
                    ))
                    .into();

                cmd.lock().spawn(root);
            }
            .instrument(tracing::debug_span!("load_assets")),
        );

        Ok(())
    }

    fn setup_objects(&mut self, world: &mut World, assets: &AssetCache) -> anyhow::Result<()> {
        self.setup_assets(world, assets)?;

        Entity::builder()
            .mount(
                TransformBundle::default()
                    .with_position(Vec3::Y * 5.0)
                    .with_rotation(Quat::from_euler(EulerRot::YXZ, 0.5, 1.0, 0.0)),
            )
            .set(light_data(), LightData::new(Srgb::new(1.0, 1.0, 1.0), 1.0))
            .set(light_kind(), LightKind::Directional)
            .set_default(cast_shadow())
            .spawn(world);

        Entity::builder()
            .mount(
                TransformBundle::default()
                    .with_position(Vec3::Y * 5.0)
                    .with_rotation(Quat::from_euler(EulerRot::YXZ, 2.0, 0.5, 0.0)),
            )
            .set(light_data(), LightData::new(Srgb::new(1.0, 1.0, 1.0), 1.0))
            .set(light_kind(), LightKind::Directional)
            .set_default(cast_shadow())
            .spawn(world);

        Entity::builder()
            .mount(TransformBundle::default().with_position(vec3(0.0, 2.0, 0.0)))
            .set(light_data(), LightData::new(Srgb::new(1.0, 0.0, 0.0), 25.0))
            .set(light_kind(), LightKind::Point)
            .spawn(world);

        Entity::builder()
            .mount(TransformBundle::default().with_position(vec3(2.0, 2.0, 5.0)))
            .set(light_data(), LightData::new(Srgb::new(0.0, 0.0, 1.0), 25.0))
            .set(light_kind(), LightKind::Point)
            .spawn(world);
        Ok(())
    }
}

impl Layer for LogicLayer {
    fn register(
        &mut self,
        world: &mut World,
        _: &AssetCache,
        mut events: EventRegisterContext<Self>,
    ) -> anyhow::Result<()> {
        events.subscribe(|this, world, assets, InitEvent| this.setup_objects(world, assets));

        events.subscribe(|_, world, _, resized: &ResizedEvent| {
            if let Some(main_camera) = Query::new(projection_matrix().as_mut())
                .with(main_camera())
                .borrow(world)
                .first()
            {
                let aspect =
                    resized.physical_size.width as f32 / resized.physical_size.height as f32;
                tracing::info!(%aspect);
                *main_camera = Mat4::perspective_rh(1.0, aspect, 0.1, 1000.0);
            }

            Ok(())
        });

        setup_camera()
            .set(
                environment_data(),
                EnvironmentData::new(
                    Srgb::new(0.2, 0.2, 0.3),
                    0.001,
                    if ENABLE_SKYBOX { 0.0 } else { 1.0 },
                ),
            )
            .spawn(world);

        Ok(())
    }
}

fn animate_system() -> BoxedSystem {
    System::builder()
        .with_query(Query::new((
            animator().as_mut(),
            delta_time()
                .source(engine())
                .expect("delta_time must be present"),
        )))
        .par_for_each(move |(animator, dt)| {
            animator.step(dt.as_secs_f32());
        })
        .boxed()
}

fn point_light_gizmo_system() -> BoxedSystem {
    System::builder()
        .with_query(Query::new(ivy_core::components::gizmos().source(engine())))
        .with_query(Query::new((world_transform(), light_data(), light_kind())))
        .build(
            |mut gizmos: QueryBorrow<flax::fetch::Source<Component<gizmos::Gizmos>, Entity>>,
             mut query: QueryBorrow<(
                Component<Mat4>,
                Component<LightData>,
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
                        LightKind::Directional => {
                            let pos = transform.transform_point3(Vec3::ZERO);
                            let dir = transform.transform_vector3(Vec3::Z);

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

use std::f32::consts::PI;

use anyhow::Context;
use flax::{
    component,
    components::name,
    filter::{All, With},
    CommandBuffer, Component, ComponentMut, Entity, EntityRef, Query, QueryBorrow, System, World,
};
use glam::{vec3, EulerRot, Quat, Vec2, Vec3};
use ivy_assets::{fs::AssetPath, stored::DynamicStore, AssetCache};
use ivy_core::{
    palette::Srgb,
    profiling::ProfilingLayer,
    transforms::TransformUpdatePlugin,
    update_layer::{FixedTimeStep, Plugin, ScheduledLayer},
    App, EngineLayer, EntityBuilderExt, DEG_45,
};
use ivy_engine::{engine, gizmos, main_camera, RigidBodyBundle, TransformBundle};
use ivy_game::{
    camera::{self, CameraQuery},
    fly_camera::FlyCameraPlugin,
    viewport_camera::{CameraSettings, ViewportCameraLayer},
};
use ivy_graphics::texture::TextureData;
use ivy_input::{
    components::input_state, layer::InputLayer, Action, CursorPositionBinding, InputState,
    KeyBinding, MouseButtonBinding,
};
use ivy_physics::{components::physics_state, ColliderBundle, GizmoSettings, PhysicsPlugin};
use ivy_postprocessing::preconfigured::{
    pbr::{PbrRenderGraphConfig, SkyboxConfig},
    SurfacePbrPipelineDesc, SurfacePbrRenderer,
};
use ivy_scene::editor::{
    hierarchy_panel::HierarchyPanel,
    manipulator::{ManipulatedEntity, ManipulationSpace, SnapMode, TransformController},
};
use ivy_ui::{
    layer::{UiLayer, UiUpdateLayer},
    screens::{screen_state, Screen, ScreenLifetimeToken},
    streamed::StreamedUiPlugin,
};
use ivy_wgpu::{
    components::{cast_shadow, forward_pass, light_kind, light_params},
    driver::WinitDriver,
    layer::GraphicsLayer,
    light::{LightKind, LightParams},
    material_desc::{MaterialData, PbrMaterialData},
    mesh_desc::MeshDesc,
    primitives::{CapsulePrimitive, CubePrimitive},
    renderer::{EnvironmentData, RenderObjectBundle},
};
use rand::{rngs::StdRng, Rng, SeedableRng};
use rand_distr::UnitSphere;
use rapier3d::prelude::{QueryFilter, SharedShape};
use tracing_subscriber::{layer::SubscriberExt, registry, util::SubscriberInitExt, EnvFilter};
use tracing_tree::HierarchicalLayer;
use violet::{
    core::{
        components::LayoutAlignment,
        layout::Align,
        style::SizeExt,
        widget::{card, label, maximized},
        Scope, Widget,
    },
    palette::Srgba,
};
use wgpu::TextureFormat;
use winit::{
    dpi::LogicalSize,
    keyboard::{Key, NamedKey},
    window::WindowAttributes,
};

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
                .with_inner_size(LogicalSize::new(1280, 720))
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
                .with_plugin(FlyCameraPlugin)
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

fn setup_objects(world: &mut World, assets: &AssetCache) -> anyhow::Result<()> {
    Entity::builder()
        .mount(TransformBundle::default().with_rotation(Quat::from_euler(
            EulerRot::YXZ,
            -2.0,
            -1.0,
            0.0,
        )))
        .set(
            light_params(),
            LightParams::new(Srgb::new(1.0, 1.0, 1.0), 1.0),
        )
        .set(light_kind(), LightKind::Directional)
        .set_default(cast_shadow())
        .spawn(world);

    let material = MaterialData::PbrMaterial(
        PbrMaterialData::new()
            .with_roughness_factor(0.1)
            .with_metallic_factor(0.0)
            .with_albedo(TextureData::srgba(Srgba::new(1.0, 1.0, 1.0, 1.0))),
    );

    let cube_mesh = MeshDesc::Content(assets.load(&CubePrimitive));
    let capsule_mesh = MeshDesc::Content(assets.load(&CapsulePrimitive::default()));

    let body = |position: Vec3, rotation: Quat| {
        let mut builder = Entity::builder();
        builder
            .mount(
                TransformBundle::default()
                    .with_position(position)
                    .with_rotation(rotation),
            )
            .mount(
                RigidBodyBundle::dynamic()
                    .with_linear_damping(0.0)
                    .with_angular_damping(0.1),
            );

        builder
    };

    let cube = |position: Vec3, rotation: Quat| {
        let mut builder = body(position, rotation);
        builder
            .set(name(), "Cube".into())
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

    let capsule = |position: Vec3, rotation: Quat| {
        let mut builder = body(position, rotation);
        builder
            .set(name(), "Capsule".into())
            .mount(
                ColliderBundle::new(SharedShape::cuboid(1.0, 1.0, 1.0))
                    .with_friction(0.7)
                    .with_restitution(0.1),
            )
            .mount(RenderObjectBundle::new(
                capsule_mesh.clone(),
                &[(forward_pass(), material.clone())],
            ));
        builder
    };

    let mut rng = StdRng::seed_from_u64(42);
    for _ in 0..100 {
        let v = Vec3::from_array(rng.sample(UnitSphere)) * 20.0;
        cube(v, Quat::IDENTITY).spawn(world);
    }

    cube(
        vec3(2.0, 0.0, -0.99),
        Quat::from_scaled_axis(vec3(0.0, 0.0, 0.5)),
    )
    .spawn(world);

    capsule(
        vec3(-2.0, 0.0, -0.99),
        Quat::from_scaled_axis(vec3(0.0, 0.0, -0.5)),
    )
    .spawn(world);

    Ok(())
}

struct ExamplePlugin;

impl Plugin for ExamplePlugin {
    fn install(
        &self,
        world: &mut World,
        assets: &AssetCache,
        _: &mut DynamicStore,
        schedules: &mut ivy_core::update_layer::ScheduleSetBuilder,
    ) -> anyhow::Result<()> {
        world.get(engine(), screen_state())?.open(MainUi);

        setup_objects(world, assets)?;

        component! {
            transform_controller: TransformController,
            shift_input: bool,
            cursor_position: Vec2,
        }

        let mut main_camera_query = Query::new(CameraQuery::new()).with(main_camera());
        let mouse_button_changed =
            move |entity: &EntityRef, _: &mut CommandBuffer, pressed: bool| -> anyhow::Result<()> {
                let mut manipulator = entity.get_mut(transform_controller())?;

                let world = entity.world();
                if pressed {
                    let mut camera = main_camera_query.borrow(entity.world());
                    let camera = camera.first().context("No main camera")?;

                    let cursor_pos = entity.get_copy(cursor_position())?;
                    let ray = camera::screen_to_world_ray(cursor_pos, camera);

                    if manipulator.try_start_move(ray, world) {
                        return Ok(());
                    }

                    let physics = world.get(engine(), physics_state())?;

                    let hit = physics.cast_ray(ray, 100.0, true, QueryFilter::new());

                    if !entity.get_copy(shift_input())? {
                        manipulator.clear_entities();
                    }

                    if let Some(hit) = hit {
                        manipulator.toggle_entity(ManipulatedEntity::from_entity(
                            world.entity(hit.rigidbody_id)?,
                        ));
                    }
                } else {
                    manipulator.finish_move(world);
                }

                anyhow::Ok(())
            };

        let mut main_camera_query = Query::new(CameraQuery::new()).with(main_camera());
        let mouse_moved = move |entity: &EntityRef, _: &mut CommandBuffer, pos: Vec2| {
            let mut manipulator = entity.get_mut(transform_controller())?;

            let mut camera = main_camera_query.borrow(entity.world());
            let camera = camera.first().context("No main camera")?;

            let ray = camera::screen_to_world_ray(pos, camera);

            manipulator.handle_mouse_move(ray, entity.world())?;
            anyhow::Ok(())
        };

        let input = InputState::new()
            .with_action(
                shift_input(),
                Action::new().with_binding(KeyBinding::new(Key::Named(NamedKey::Shift))),
            )
            .with_trigger_action(
                Action::new().with_binding(KeyBinding::new(Key::Character("g".into()))),
                |entity: &EntityRef, _: &mut CommandBuffer, pressed: bool| {
                    if !pressed {
                        return Ok(());
                    }

                    let mut manipulator = entity.get_mut(transform_controller())?;
                    let space = if manipulator.space == ManipulationSpace::Global {
                        ManipulationSpace::Local
                    } else {
                        ManipulationSpace::Global
                    };

                    manipulator.set_space(space);
                    Ok(())
                },
            )
            .with_trigger_action(
                Action::new().with_binding(KeyBinding::new(Key::Character("v".into()))),
                |entity: &EntityRef, _: &mut CommandBuffer, pressed: bool| {
                    if !pressed {
                        return Ok(());
                    }

                    let mut manipulator = entity.get_mut(transform_controller())?;
                    manipulator.set_space(ManipulationSpace::View);
                    Ok(())
                },
            )
            .with_trigger_action(
                Action::new().with_binding(KeyBinding::new(Key::Character("l".into()))),
                |entity: &EntityRef, _: &mut CommandBuffer, pressed: bool| {
                    if !pressed {
                        return Ok(());
                    }

                    let mut manipulator = entity.get_mut(transform_controller())?;

                    tracing::info!(current_snap_mode = ?manipulator.snap_mode());

                    if manipulator.snap_mode().is_absolute() {
                        manipulator.set_snap_mode(SnapMode::None);
                        manipulator.set_angle_snap(0.0);
                    } else {
                        manipulator.set_snap_mode(SnapMode::Absolute(1.0));
                        manipulator.set_angle_snap(PI / 180.0 * 5.0);
                    }

                    tracing::info!("New Snap mode: {:?}", manipulator.snap_mode());

                    Ok(())
                },
            )
            .with_trigger_action(
                Action::new()
                    .with_binding(MouseButtonBinding::new(winit::event::MouseButton::Left)),
                mouse_button_changed,
            )
            .with_action(
                cursor_position(),
                Action::new().with_binding(CursorPositionBinding::new(true)),
            )
            .with_trigger_action(
                Action::new().with_binding(CursorPositionBinding::new(true)),
                mouse_moved,
            );

        let manipulator_entity = Entity::builder()
            .set(name(), "Manipulator".into())
            .set(transform_controller(), TransformController::new(vec![]))
            .set(input_state(), input)
            .spawn(world);

        schedules.per_tick_mut().with_system(
            System::builder()
                .with_world()
                .with_query(Query::new(CameraQuery::new()).with(main_camera()))
                .with_query(Query::new((
                    transform_controller().as_mut(),
                    cursor_position(),
                )))
                .build(
                    move |world: &World,
                          mut camera_query: QueryBorrow<CameraQuery, (All, With)>,
                          mut query: QueryBorrow<(
                        ComponentMut<TransformController>,
                        Component<Vec2>,
                    )>| {
                        let gizmos = world.get(engine(), gizmos())?;

                        let camera = camera_query.first().context("No main camera")?;

                        let mut gizmos = gizmos.begin_section("example_manipulator_system");
                        for (manipulator, cursor_pos) in &mut query {
                            let ray = camera::screen_to_world_ray(*cursor_pos, camera);
                            manipulator.update(world, Quat::from_mat4(camera.transform));
                            manipulator.draw(&mut gizmos, ray);
                        }

                        anyhow::Ok(())
                    },
                ),
        );

        Ok(())
    }
}

struct MainUi;

impl Screen for MainUi {
    fn create(self, scope: &mut Scope<'_>, _: ScreenLifetimeToken) {
        maximized(card((
            label("Transforms").with_item_align(LayoutAlignment::new(Align::Center, Align::Start)),
            // HierarchyPanel::new()
            //     .with_item_align(LayoutAlignment::new(Align::Start, Align::Center)),
        )))
        .mount(scope);
    }
}

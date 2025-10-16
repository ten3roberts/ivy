use flax::{components::name, fetch::MutGuard, system, Entity, FetchExt, World};
use glam::{vec3, EulerRot, Quat, Vec2, Vec3};
use ivy_assets::{stored::DynamicStore, AssetCache};
use ivy_core::{
    components::{engine, main_camera, request_capture_mouse, rotation, world_transform},
    math::{Axis2D, Axis3D},
    plugin::{Plugin, PluginContext},
    Bundle, EntityBuilderExt, DEG_90,
};
use ivy_graphics::camera::CameraBundle;
use ivy_input::{
    components::input_state, types::MouseButton, Action, BindingExt, CompositeBinding,
    CursorMoveBinding, InputState, MouseButtonBinding, ScrollBinding,
};

flax::component! {
    control_active: bool,
    rotation_input: Vec2,
    theta: f32,
    phi: f32,
    focus_point: Vec3,
    distance: f32,
    distance_input: f32,
    pan_input:Vec2,
}

pub struct OrbitCameraPlugin;

impl Plugin for OrbitCameraPlugin {
    fn install(&self, ctx: PluginContext) -> anyhow::Result<()> {
        let PluginContext { world, assets, store, schedules } = ctx;
        Entity::builder().mount(OrbitCameraBundle).spawn(world);

        schedules
            .per_tick_mut()
            .with_system(lock_cursor_system())
            .with_system(camera_orbit_system())
            .with_system(camera_pan_system())
            .with_system(update_camera_position_system());

        Ok(())
    }
}

struct OrbitCameraBundle;

impl Bundle for OrbitCameraBundle {
    fn mount(&self, entity: &mut flax::EntityBuilder) {
        let control_action = Action::new()
            .with_binding(MouseButtonBinding::new(MouseButton::Left))
            .with_binding(MouseButtonBinding::new(MouseButton::Right));

        let rotate_action = Action::new().with_binding(CompositeBinding::new(
            CursorMoveBinding::new().amplitude(Vec2::ONE * -0.001),
            vec![MouseButtonBinding::new(MouseButton::Right)],
        ));

        let pan_action = Action::new().with_binding(CompositeBinding::new(
            CursorMoveBinding::new().amplitude(Vec2::ONE * 0.0005),
            vec![MouseButtonBinding::new(MouseButton::Left)],
        ));

        let distance_action =
            Action::new().with_binding(ScrollBinding::new().decompose(Axis2D::Y).amplitude(-1.0));

        entity
            .mount(TransformBundle::default())
            .mount(CameraBundle::default())
            .set(name(), "OrbitCamera".to_string())
            .set(main_camera(), ())
            .set(phi(), -0.5)
            .set_default(theta())
            .set_default(focus_point())
            .set(
                input_state(),
                InputState::new()
                    .with_action(rotation_input(), rotate_action)
                    .with_action(distance_input(), distance_action)
                    .with_action(pan_input(), pan_action)
                    .with_action(control_active(), control_action),
            )
            .set(distance(), 15.0);
    }
}

#[system(args(control_active=control_active().modified(), request_capture_mouse=request_capture_mouse().maybe_mut().source(engine())))]
fn lock_cursor_system(control_active: &bool, request_capture_mouse: MutGuard<bool>) {
    *request_capture_mouse.write() = *control_active;
}

#[system]
fn camera_orbit_system(
    theta: &mut f32,
    phi: &mut f32,
    distance: &mut f32,
    rotation_input: Vec2,
    distance_input: f32,
) {
    *theta += rotation_input.x;
    *phi = (*phi + rotation_input.y).clamp(-DEG_90, DEG_90);
    *distance = (*distance * (2_f32.powf(distance_input * 0.02))).clamp(0.1, 1000.0);
}

#[system]
fn camera_pan_system(focus_point: &mut Vec3, rotation: Quat, pan_input: Vec2, distance: f32) {
    let delta = rotation * vec3(-pan_input.x, pan_input.y, 0.0) * distance;
    *focus_point += delta;
}

#[system]
fn update_camera_position_system(
    position: &mut Vec3,
    rotation: &mut Quat,
    focus_point: Vec3,
    theta: f32,
    phi: f32,
    distance: f32,
) {
    *rotation = Quat::from_euler(EulerRot::YXZ, theta, phi, 0.0);
    *position = focus_point + (*rotation * Vec3::Z * distance);
}

use flax::{
    BoxedSystem, Component, Entity, FetchExt, Mutable, Query, QueryBorrow, ScheduleBuilder, System,
    World,
};
use glam::{vec3, EulerRot, Quat, Vec2, Vec3};
use ivy_assets::AssetCache;
use ivy_core::{
    components::{main_camera, rotation, velocity, TransformBundle},
    update_layer::{PerTick, Plugin},
    EntityBuilderExt, DEG_45,
};
use ivy_input::{
    components::input_state,
    types::{Key, NamedKey},
    Action, Axis2D, BindingExt, CursorMovement, InputState, KeyBinding, MouseButtonBinding,
    ScrollBinding,
};
use ivy_wgpu::{
    components::{main_window, projection_matrix, window},
    driver::WindowHandle,
};

flax::component! {
    pub pan_active: f32,
    pub rotation_input: Vec2,
    pub euler_rotation: Vec3,
    pub camera_movement: Vec3,
    pub camera_speed: f32,
    pub camera_speed_delta: f32,
}

pub struct CameraInputPlugin;

impl Plugin<PerTick> for CameraInputPlugin {
    fn install(
        &self,
        _: &mut World,
        _: &AssetCache,
        schedule: &mut ScheduleBuilder,
        _: &PerTick,
    ) -> anyhow::Result<()> {
        schedule
            .with_system(cursor_lock_system())
            .with_system(camera_speed_input_system())
            .with_system(camera_rotation_input_system())
            .with_system(camera_movement_input_system());

        Ok(())
    }
}

pub fn setup_camera() -> flax::EntityBuilder {
    let mut speed_action = Action::new(camera_speed_delta());
    speed_action.add(ScrollBinding::new().decompose(Axis2D::Y));

    let mut move_action = Action::new(camera_movement());
    move_action.add(KeyBinding::new(Key::Character("w".into())).compose(Vec3::Z));
    move_action.add(KeyBinding::new(Key::Character("a".into())).compose(-Vec3::X));
    move_action.add(KeyBinding::new(Key::Character("s".into())).compose(-Vec3::Z));
    move_action.add(KeyBinding::new(Key::Character("d".into())).compose(Vec3::X));

    move_action.add(KeyBinding::new(Key::Character("c".into())).compose(-Vec3::Y));
    move_action.add(KeyBinding::new(Key::Named(NamedKey::Control)).compose(-Vec3::Y));
    move_action.add(KeyBinding::new(Key::Named(NamedKey::Space)).compose(Vec3::Y));

    let mut rotate_action = Action::new(rotation_input());
    rotate_action.add(CursorMovement::new().amplitude(Vec2::ONE * 0.001));

    let mut pan_action = Action::new(pan_active());
    pan_action
        .add(KeyBinding::new(Key::Character("q".into())))
        .add(MouseButtonBinding::new(
            ivy_input::types::MouseButton::Right,
        ));

    let mut builder = Entity::builder();
    builder
        .mount(TransformBundle::new(
            vec3(0.0, 10.0, 10.0),
            Quat::IDENTITY,
            Vec3::ONE,
        ))
        .set(main_camera(), ())
        .set_default(projection_matrix())
        .set_default(velocity())
        .set(
            input_state(),
            InputState::new()
                .with_action(move_action)
                .with_action(rotate_action)
                .with_action(pan_action)
                .with_action(speed_action),
        )
        .set_default(camera_movement())
        .set_default(rotation_input())
        .set(euler_rotation(), vec3(DEG_45, 0.0, 0.0))
        .set_default(pan_active())
        .set(camera_speed(), 5.0)
        .set_default(camera_speed_delta());

    builder
}

fn cursor_lock_system() -> BoxedSystem {
    System::builder()
        .with_query(Query::new(pan_active()))
        .with_query(Query::new(window().as_mut()).with(main_window()))
        .build(
            |mut query: QueryBorrow<Component<f32>>,
             mut window: QueryBorrow<Mutable<WindowHandle>, _>| {
                query.iter().for_each(|&pan_active| {
                    if let Some(window) = window.first() {
                        window.set_cursor_lock(pan_active > 0.0);
                    }
                });
            },
        )
        .boxed()
}

fn camera_speed_input_system() -> BoxedSystem {
    System::builder()
        .with_query(Query::new((
            camera_speed().as_mut(),
            camera_speed_delta().modified(),
        )))
        .for_each(|(speed, &delta)| {
            let change = 2_f32.powf(delta * 0.05);
            *speed = (*speed * change).clamp(0.1, 1000.0);
            tracing::info!("camera speed: {speed} {delta}");
        })
        .boxed()
}

fn camera_rotation_input_system() -> BoxedSystem {
    System::builder()
        .with_query(Query::new((
            rotation().as_mut(),
            euler_rotation().as_mut(),
            rotation_input(),
            pan_active(),
        )))
        .for_each(|(rotation, euler_rotation, rotation_input, &pan_active)| {
            *euler_rotation += pan_active * vec3(rotation_input.y, rotation_input.x, 0.0);
            *rotation = Quat::from_euler(EulerRot::YXZ, -euler_rotation.y, -euler_rotation.x, 0.0);
        })
        .boxed()
}

fn camera_movement_input_system() -> BoxedSystem {
    System::builder()
        .with_query(Query::new((
            camera_movement(),
            rotation(),
            camera_speed(),
            velocity().as_mut(),
        )))
        .for_each(move |(&movement, rotation, &camera_speed, velocity)| {
            *velocity = *rotation * (movement * vec3(1.0, 1.0, -1.0) * camera_speed);
        })
        .boxed()
}
use std::time::Instant;

use flax::{
    component, BoxedSystem, Component, Entity, FetchExt, Mutable, Query, QueryBorrow, Schedule,
    System, World,
};
use glam::{vec3, EulerRot, Mat4, Quat, Vec2, Vec3};
use ivy_assets::AssetCache;
use ivy_base::{
    app::{InitEvent, TickEvent},
    engine,
    layer::events::EventRegisterContext,
    main_camera, position, rotation, App, EngineLayer, EntityBuilderExt, Layer, TransformBundle,
};
use ivy_input::{
    components::input_state, layer::InputLayer, types::Key, Action, Axis3, BindingExt, Compose,
    CursorMovement, InputState, KeyBinding, MouseButtonBinding,
};
use ivy_wgpu::{
    components::{main_window, projection_matrix, window},
    driver::{WindowHandle, WinitDriver},
    events::ResizedEvent,
    layer::GraphicsLayer,
    material::MaterialDesc,
    mesh::{MeshData, MeshDesc},
    renderer::RenderObjectBundle,
    shader::ShaderDesc,
    texture::TextureDesc,
};
use tracing_subscriber::{layer::SubscriberExt, registry, util::SubscriberInitExt, EnvFilter};
use tracing_tree::HierarchicalLayer;

pub fn main() -> anyhow::Result<()> {
    registry()
        .with(EnvFilter::from_default_env())
        .with(HierarchicalLayer::default().with_indent_lines(true))
        .init();

    let dt = 0.02;

    if let Err(err) = App::builder()
        .with_driver(WinitDriver::new())
        .with_layer(EngineLayer::new())
        .with_layer(GraphicsLayer::new())
        .with_layer(InputLayer::new())
        .with_layer(LogicLayer::new())
        .with_layer(Update::new(
            Schedule::builder()
                .with_system(cursor_lock_system())
                .with_system(update_camera_rotation_system())
                .with_system(read_input_rotation_system())
                .build(),
        ))
        .with_layer(FixedUpdate::new(
            dt,
            Schedule::builder().with_system(movement_system(dt)).build(),
        ))
        .run()
    {
        tracing::error!("{err:?}");
        Err(err)
    } else {
        Ok(())
    }
}

pub struct LogicLayer {
    entity: Option<Entity>,
}

impl Default for LogicLayer {
    fn default() -> Self {
        Self::new()
    }
}

impl LogicLayer {
    pub fn new() -> Self {
        Self { entity: None }
    }

    fn setup_objects(&mut self, world: &mut World, assets: &AssetCache) -> anyhow::Result<()> {
        let shader = assets.insert(ShaderDesc::new(
            "diffuse",
            include_str!("../assets/shaders/diffuse.wgsl"),
        ));

        let mesh = assets.insert(MeshDesc::content(assets.insert(MeshData::quad())));

        let material = assets.insert(MaterialDesc::new(TextureDesc::path(
            "assets/textures/statue.jpg",
        )));

        let material2 = assets.insert(MaterialDesc::new(TextureDesc::path(
            "assets/textures/grid.png",
        )));

        Entity::builder()
            .mount(RenderObjectBundle::new(
                mesh.clone(),
                material.clone(),
                shader.clone(),
            ))
            .mount(TransformBundle::new(
                vec3(0.0, 0.0, 2.0),
                Quat::IDENTITY,
                Vec3::ONE,
            ))
            .spawn(world);

        let entity = Entity::builder()
            .mount(RenderObjectBundle::new(mesh, material2, shader))
            .mount(TransformBundle::new(
                vec3(1.0, 0.0, 2.0),
                Quat::from_euler(glam::EulerRot::ZYX, 1.0, 0.0, 0.0),
                Vec3::ONE,
            ))
            .spawn(world);

        self.entity = Some(entity);

        Ok(())
    }
}

impl Layer for LogicLayer {
    fn register(
        &mut self,
        world: &mut World,
        assets: &AssetCache,
        mut events: EventRegisterContext<Self>,
    ) -> anyhow::Result<()> {
        events.subscribe(|this, world, assets, InitEvent| this.setup_objects(world, assets));

        let start_time = std::time::Instant::now();
        events.subscribe(move |this, world, _, _: &TickEvent| {
            let t = start_time.elapsed().as_secs_f32();
            if let Some(entity) = this.entity {
                world
                    .set(entity, rotation(), Quat::from_axis_angle(Vec3::Y, t))
                    .unwrap();
            }
            Ok(())
        });

        events.subscribe(|_, world, _, resized: &ResizedEvent| {
            if let Some(main_camera) = Query::new(projection_matrix().as_mut())
                .with(main_camera())
                .borrow(world)
                .first()
            {
                let aspect =
                    resized.physical_size.width as f32 / resized.physical_size.height as f32;
                tracing::info!(%aspect);
                *main_camera = Mat4::perspective_lh(1.0, aspect, 0.1, 1000.0);
            }

            Ok(())
        });

        let mut move_action = Action::new(movement());
        move_action.add(KeyBinding::new(Key::Character("w".into())).compose(Vec3::Z));
        move_action.add(KeyBinding::new(Key::Character("a".into())).compose(-Vec3::X));
        move_action.add(KeyBinding::new(Key::Character("s".into())).compose(-Vec3::Z));
        move_action.add(KeyBinding::new(Key::Character("d".into())).compose(Axis3::X));

        let mut rotate_action = Action::new(rotation_input());
        rotate_action.add(CursorMovement::new().amplitude(Vec2::ONE * 0.001));

        let mut pan_action = Action::new(pan_active());
        pan_action
            .add(KeyBinding::new(Key::Character("q".into())))
            .add(MouseButtonBinding::new(
                ivy_input::types::MouseButton::Right,
            ));

        Entity::builder()
            .mount(TransformBundle::new(Vec3::ZERO, Quat::IDENTITY, Vec3::ONE))
            .set(main_camera(), ())
            .set_default(projection_matrix())
            .set(
                input_state(),
                InputState::new()
                    .with_action(move_action)
                    .with_action(rotate_action)
                    .with_action(pan_action),
            )
            .set_default(movement())
            .set_default(rotation_input())
            .set_default(euler_rotation())
            .set_default(pan_active())
            .set(camera_speed(), 1.0)
            .spawn(world);

        Ok(())
    }
}

pub struct Update {
    schedule: Schedule,
    current_time: Instant,
}

impl Update {
    pub fn new(schedule: Schedule) -> Self {
        Self {
            schedule,
            current_time: Instant::now(),
        }
    }

    pub fn tick(&mut self, world: &mut World) -> anyhow::Result<()> {
        let now = Instant::now();

        let _elapsed = now.duration_since(self.current_time);
        self.current_time = now;

        self.schedule.execute_par(world)?;

        Ok(())
    }
}

impl Layer for Update {
    fn register(
        &mut self,
        _: &mut World,
        _: &AssetCache,
        mut events: EventRegisterContext<Self>,
    ) -> anyhow::Result<()>
    where
        Self: Sized,
    {
        events.subscribe(|this, world, _, _: &TickEvent| this.tick(world));

        Ok(())
    }
}
pub struct FixedUpdate {
    dt: f32,
    schedule: Schedule,
    acc: f64,
    current_time: Instant,
}

impl FixedUpdate {
    pub fn new(dt: f32, schedule: Schedule) -> Self {
        Self {
            dt,
            schedule,
            acc: 0.0,
            current_time: Instant::now(),
        }
    }

    pub fn tick(&mut self, world: &mut World) -> anyhow::Result<()> {
        let now = Instant::now();

        let elapsed = now.duration_since(self.current_time);
        self.current_time = now;

        self.acc += elapsed.as_secs_f64();

        while self.acc > self.dt as f64 {
            self.schedule.execute_par(world)?;
            self.acc -= self.dt as f64;
        }

        Ok(())
    }
}

impl Layer for FixedUpdate {
    fn register(
        &mut self,
        _: &mut World,
        _: &AssetCache,
        mut events: EventRegisterContext<Self>,
    ) -> anyhow::Result<()>
    where
        Self: Sized,
    {
        events.subscribe(|this, world, _, _: &TickEvent| this.tick(world));

        Ok(())
    }
}

component! {
    pan_active: f32,
    rotation_input: Vec2,
    euler_rotation: Vec3,
    movement: Vec3,
    camera_speed: f32,
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

fn read_input_rotation_system() -> BoxedSystem {
    System::builder()
        .with_query(Query::new((euler_rotation().as_mut(), rotation_input())))
        .for_each(|(rotation, rotation_input)| {
            *rotation += vec3(rotation_input.y, rotation_input.x, 0.0);
        })
        .boxed()
}

fn update_camera_rotation_system() -> BoxedSystem {
    System::builder()
        .with_query(Query::new((rotation().as_mut(), euler_rotation())))
        .for_each(|(rotation, euler_rotation)| {
            *rotation = Quat::from_euler(EulerRot::YXZ, euler_rotation.y, euler_rotation.x, 0.0);
        })
        .boxed()
}

fn movement_system(dt: f32) -> BoxedSystem {
    System::builder()
        .with_query(Query::new((
            movement(),
            rotation(),
            camera_speed(),
            position().as_mut(),
        )))
        .for_each(move |(&movement, rotation, &camera_speed, position)| {
            *position += *rotation * movement * camera_speed * dt;
        })
        .boxed()
}

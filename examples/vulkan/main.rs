#![allow(dead_code)]
mod movement;

use std::{fmt::Display, ops::DerefMut, time::Duration};

use anyhow::Context;
use collision::{components::collider, util::project_plane, BvhNode, Collider, Cube, Ray, Sphere};
use flax::{
    components::child_of, entity_ids, BoxedSystem, Entity, EntityBuilder, FetchExt, Query,
    Schedule, System, World,
};
use flume::Receiver;
use glam::{vec2, vec3, Quat, Vec2, Vec2Swizzles, Vec3};
use glfw::{CursorMode, Key, MouseButton, WindowEvent};
use graphics::components::{bounding_sphere, camera, light_source};
use input::components::input_state;
use ivy_assets::{Asset, AssetCache};
use ivy_engine::{base::*, graphics::*, input::*, ui::*, vulkan::*, *};
use ivy_wgpu::layer::GraphicsLayerDesc;
use movement::{move_system, mover, Mover};
use physics::{
    components::{collision_state, collision_tree, effector},
    connections::draw_connections,
    PhysicsLayerDesc,
};
use presets::{
    default_geometry_shader, default_gizmo_shader, default_image_shader,
    default_post_processing_shader, default_text_shader, default_transparent_shader, geometry_pass,
    text_pass, ui_pass, PBRRenderingInfo,
};
use random::rand::SeedableRng;
use random::{rand::rngs::StdRng, Random};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};
use vulkan::vk::{CullModeFlags, PresentModeKHR};

const FRAMES_IN_FLIGHT: usize = 2;

type CollisionNode = BvhNode;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(EnvFilter::from_default_env())
        .with(tracing_tree::HierarchicalLayer::default())
        .init();

    // Go up three levels
    ivy_base::normalize_dir(3)?;

    let window = WindowInfo {
        resizable: false,
        mode: WindowMode::Windowed(Extent::new(1280, 720)),
        ..Default::default()
    };

    let swapchain = SwapchainInfo {
        present_mode: PresentModeKHR::IMMEDIATE,
        image_count: FRAMES_IN_FLIGHT as u32 + 1,
        ..Default::default()
    };

    let result = App::builder()
        .with_layer(EngineLayerDesc)?
        .with_layer(WindowLayerDesc { window, swapchain })?
        // .push_layer(FixedTimeStepDefinition(
        //     20.ms(),
        //     (
        //         PhysicsLayerDesc::new(
        //             Vec3::Y * -9.80,
        //             CollisionNode::new(
        //                 collision::BoundingBox::new(Vec3::ONE * 200.0, Vec3::ZERO),
        //                 Default::default(),
        //             ),
        //             false,
        //         ),
        //         LogicLayerDesc,
        //     ),
        // ))?
        .with_layer(GraphicsLayerDesc)?
        .with_layer(DebugLayerDesc)?
        .run();

    if let Err(err) = result {
        tracing::error!("{err:?}");
        return Err(err);
    }

    Ok(())
}

fn setup_graphics(world: &mut World, assets: &AssetCache) -> anyhow::Result<AssetPack> {
    let text_shader = Shader::new(assets.insert(default_text_shader()));
    let ui_shader = Shader::new(assets.insert(default_image_shader()));
    let post_processing_shader = Shader::new(assets.insert(default_post_processing_shader()));
    let geometry_shader = Shader::new(assets.insert(default_geometry_shader(CullModeFlags::BACK)));

    let transparent_shader = Shader::new(assets.insert(default_transparent_shader()));

    let gizmo_shader = Shader::new(assets.insert(default_gizmo_shader()));

    let mut pbr = presets::PBRRendering::setup(
        world,
        assets,
        DefaultEnvData {
            ambient_radiance: Vec3::ONE * 0.5,
            fog_density: 0.01,
            fog_color: Vec3::new(0.1, 0.1, 0.1),
            fog_gradient: 2.0,
        },
        FRAMES_IN_FLIGHT,
        PBRRenderingInfo {
            color_usage: ImageUsage::COLOR_ATTACHMENT
                | ImageUsage::SAMPLED
                | ImageUsage::TRANSFER_SRC,
            text_shader: text_shader.clone(),
            ui_shader: ui_shader.clone(),
            post_processing_shader,
            gizmo_shader,
        },
    )?;

    pbr.using_swapchain(world, assets)?;
    pbr.install(world);
    // .setup_pipelines(resources, presets::PipelinesInfo::default())?;

    Ok(AssetPack {
        geometry_shader,
        transparent_shader,
        text_shader,
        ui_shader,
    })
}

fn setup_objects(
    world: &mut World,
    assets: &AssetCache,
    asset_pack: &AssetPack,
    camera: Entity,
    canvas: Entity,
) -> anyhow::Result<Entities> {
    let _scope = TimedScope::new(|elapsed| eprintln!("Object setup took {:.3?}", elapsed));
    world.set(engine(), gizmos(), Default::default())?;

    let cube_document: Asset<Document> =
        assets.load(&DocumentFromPath("./res/models/cube.glb".into()));

    let cube_mesh = cube_document.mesh(0);
    let material = assets.load(&MaterialInfo {
        albedo: "./res/textures/metal.png".into(),
        normal: Some("./res/textures/metal_normal.png".into()),
        sampler: SamplerKey::default(),
        roughness: 0.1,
        metallic: 1.0,
    });

    let sphere_document = assets.load(&DocumentFromPath("./res/models/sphere.gltf".into()));

    let sphere_mesh = sphere_document.mesh(0);

    let mut builder = Entity::builder();

    builder
        .set(position(), vec3(0.0, 5.0, 5.0))
        .set(
            light_source(),
            PointLight::new(1.0, vec3(1.0, 1.0, 1.0) * 5000.0),
        )
        .spawn(world);

    let mut builder = EntityBuilder::new();

    builder
        .mount(RbBundle {
            mass: 50.0,
            ..Default::default()
        })
        .set(collider(), Collider::new(Sphere::new(1.0)))
        .mount(RenderObjectBundle {
            pos: vec3(0.0, 0.6, -1.2),
            scale: Vec3::splat(0.5),
            // pass: assets.geometry_pass,
            mesh: sphere_mesh.clone(),
            material: Some(material.clone()),
            color: Color::red(),
            rotation: Default::default(),
        })
        .set(geometry_pass(), asset_pack.geometry_shader.clone())
        .set(is_static(), ());

    let sphere = builder.spawn(world);

    let mut builder = EntityBuilder::new();
    let light = builder
        .mount(TransformBundle {
            pos: vec3(0.0, 4.0, 0.0),
            scale: Vec3::splat(0.5),
            ..Default::default()
        })
        .mount(RbBundle {
            mass: 50.0,
            ..Default::default()
        })
        // .add_bundle(ConnectionBundle::new(
        //     ConnectionKind::spring(100.0, 50.0),
        //     PositionOffset::new(0.0, 4.0, 0.0),
        //     RotationOffset::default(),
        // ))
        // .set(light_source(), PointLight::new(0.2, Vec3::ONE * 10000.0))
        .set(
            connection(sphere),
            ConnectionKind::Spring {
                strength: 100.0,
                dampening: 50.0,
            },
        )
        .spawn(world);

    let mut builder = EntityBuilder::new();
    builder
        .mount(RenderObjectBundle {
            scale: Vec3::splat(0.25),
            mesh: cube_mesh.clone(),
            material: Some(material.clone()),
            pos: Default::default(),
            rotation: Default::default(),
            color: Color::white(),
        })
        .set(geometry_pass(), asset_pack.geometry_shader.clone())
        .mount(RbBundle {
            mass: 10.0,
            ..Default::default()
        })
        .set(collider(), Collider::new(Cube::uniform(1.0)))
        .set(
            connection(light),
            ConnectionKind::Spring {
                strength: 10.0,
                dampening: 3.0,
            },
        )
        .set(position_offset(), vec3(2.0, 1.0, 0.0))
        .set(rotation_offset(), Default::default())
        .spawn(world);

    let mut builder = EntityBuilder::new();

    builder
        .mount(RenderObjectBundle {
            scale: Vec3::splat(0.25),
            mesh: sphere_mesh.clone(),
            material: Some(material.clone()),
            pos: Default::default(),
            rotation: Default::default(),
            color: Color::white(),
        })
        .set(geometry_pass(), asset_pack.geometry_shader.clone())
        .mount(RbBundle {
            // collider: Collider::new(Sphere::new(1.0)),
            mass: 10.0,
            ..Default::default()
        })
        .set(collider(), Collider::new(Sphere::new(1.0)))
        .set(connection(light), ConnectionKind::Rigid)
        .set(position_offset(), vec3(-1.0, 0.0, 2.0))
        .set_default(rotation_offset())
        .spawn(world);

    let mut rng = StdRng::seed_from_u64(42);

    const COUNT: usize = 128;

    (0..COUNT).for_each(|_| {
        let pos = Vec3::rand_uniform(&mut rng) * 100.0;
        let vel = Vec3::rand_uniform(&mut rng) * 0.1;

        builder
            .mount(RenderObjectBundle {
                mesh: cube_mesh.clone(),
                scale: Vec3::splat(0.5),
                pos,
                color: Color::white(),
                rotation: Default::default(),
                material: None,
            })
            .set(geometry_pass(), asset_pack.geometry_shader.clone())
            .mount(RbBundle {
                vel,
                mass: 20.0,
                ang_mass: 5.0,
                friction: 1.0,
                ..Default::default()
            })
            .set(collider(), Collider::new(Cube::uniform(1.0)))
            .spawn(world);
    });

    (0..COUNT).for_each(|_| {
        let pos = Vec3::rand_uniform(&mut rng) * 100.0;
        let vel = Vec3::rand_uniform(&mut rng) * 0.5;

        builder
            .mount(RenderObjectBundle {
                mesh: sphere_mesh.clone(),
                material: Some(material.clone()),
                scale: Vec3::splat(0.5),
                pos,
                color: Color::new(1.0, 1.0, 1.0, 1.0),
                rotation: Default::default(),
            })
            // Assign to a pass during rendering
            .set(geometry_pass(), asset_pack.geometry_shader.clone())
            .mount(RbBundle {
                vel,
                mass: 20.0,
                ang_mass: 5.0,
                restitution: 1.0,
                friction: 1.0,
                ..Default::default()
            })
            .set(collider(), Collider::new(Sphere::new(1.0)))
            .spawn(world);
    });

    Ok(Entities { camera, canvas })
}

struct AssetPack {
    geometry_shader: Shader,
    transparent_shader: Shader,
    text_shader: Shader,
    ui_shader: Shader,
}

struct Entities {
    camera: Entity,
    canvas: Entity,
}

struct LogicLayerDesc;

struct LogicLayer {
    camera_euler: Vec3,

    cursor_mode: CursorMode,

    schedule: Schedule,

    // window_events: Receiver<WindowEvent>,
    // graphics_events: Receiver<GraphicsEvent>,
    assets: AssetPack,
    entities: Entities,
}

impl LogicLayer {
    pub fn handle_events(&mut self, world: &mut World, assets: &AssetCache) -> anyhow::Result<()> {
        // let window = resources.get_default_mut::<Window>()?;

        // for event in self.window_events.try_iter() {
        //     if let WindowEvent::Scroll(_, scroll) = event {
        //         let mut mover = world.get_mut(self.entities.camera, mover())?;
        //         mover.speed = (mover.speed + scroll as f32 * 0.2).clamp(0.1, 20.0);
        //     }
        // }

        // for event in self.graphics_events.try_iter() {
        //     match event {
        //         GraphicsEvent::SwapchainRecreation => {
        //             setup_graphics(world, assets)?;
        //         }
        //     }
        // }
        Ok(())
    }
}

fn update_input_state_system() -> BoxedSystem {
    System::builder()
        .with_query(Query::new(input_state().as_mut()))
        .for_each(|input| {
            input.handle_events();
        })
        .boxed()
}

impl LayerDesc for LogicLayerDesc {
    type Layer = LogicLayer;

    fn register(self, world: &mut World, assets: &AssetCache) -> anyhow::Result<Self::Layer> {
        // let input = InputState::new(
        //     &world
        //         .get(engine(), ivy_graphics::components::window())
        //         .unwrap(),
        //     events,
        // );

        // let input_vector = InputVector::new(
        //     InputAxis::keyboard(Key::A, Key::D),
        //     InputAxis::keyboard(Key::Space, Key::LeftControl),
        //     InputAxis::keyboard(Key::W, Key::S),
        // );

        let mut builder = EntityBuilder::new();

        let camera = builder
            .mount(TransformBundle {
                pos: vec3(0.0, 0.0, -7.0),
                ..Default::default()
            })
            .mount(RbBundle::default())
            .set(main_camera(), ())
            .set(camera(), Camera::perspective(1.0, 1.0, 0.1, 100.0))
            // .set(
            //     mover(),
            //     Mover::new(input_vector, Default::default(), 5.0, true),
            // )
            .spawn(world);

        let canvas = EntityBuilder::new()
            // .mount(CanvasBundle::new(input.window_extent()))
            .spawn(world);

        let asset_pack = setup_graphics(world, assets).context("Failed to setup graphics")?;

        let entities = setup_objects(world, assets, &asset_pack, camera, canvas)?;

        setup_ui(world, assets, &asset_pack)?;

        // let window_events = events.subscribe();
        // let graphics_events = events.subscribe();

        // world.set(engine(), input_state(), input)?;
        let schedule = Schedule::from([move_system(), update_input_state_system()]);

        Ok(LogicLayer {
            camera_euler: Vec3::ZERO,
            entities,
            assets: asset_pack,
            // window_events,
            // graphics_events,
            cursor_mode: CursorMode::Normal,
            schedule,
        })
    }
}

impl Layer for LogicLayer {
    fn on_update(
        &mut self,
        world: &mut World,
        assets: &mut AssetCache,
        _events: &mut Events,
        _: Duration,
    ) -> anyhow::Result<()> {
        // let _scope = TimedScope::new(|elapsed| log::trace!("Logic layer took {:.3?}", elapsed));

        self.handle_events(world, assets)
            .context("Failed to handle events")?;

        self.schedule.execute_par(world)?;

        let input = &mut world.get_mut(engine(), input_state())?;

        let mut query = Query::new((camera(), position(), rotation().as_mut())).with(main_camera());
        let mut query = query.borrow(world);

        let (camera, &camera_position, camera_rotation) = query.iter().next().unwrap();

        let mut window = world.get_mut(engine(), ivy_graphics::components::window())?;

        //  Only move camera if right mouse button is held
        if input.mouse_button_down(MouseButton::Button2) {
            window.set_cursor_mode(CursorMode::Disabled);
            let mouse_movement = input.normalized_cursor_movement() * Vec2::new(1.0, -1.0);

            self.camera_euler += mouse_movement.yx().extend(0.0);
        } else {
            window.set_cursor_mode(CursorMode::Normal);
        }

        *camera_rotation = Quat::from_euler(
            glam::EulerRot::YXZ,
            self.camera_euler.y,
            self.camera_euler.x,
            self.camera_euler.z,
        );

        // Calculate cursor to world ray
        let cursor_pos = input.normalized_cursor_pos();

        let dir = camera.to_world_ray(cursor_pos);

        let ray = Ray::new(camera_position, dir);
        let mut gizmos = world.get_mut(engine(), gizmos())?;

        let tree = world.get_mut(engine(), collision_tree())?;
        let collision_state = world.get_mut(engine(), collision_state())?;

        gizmos.begin_section("Ray Casting");
        gizmos.draw(
            base::Cube::new(Vec3::ZERO, Vec3::ONE, 0.01, 1.0),
            Color::red(),
        );
        gizmos.draw(Line::new(Vec3::ZERO, Vec3::X, 0.01, 1.0), Color::blue());
        gizmos.draw(base::Sphere::new(Vec3::ZERO, 0.1), Color::blue());

        if input.mouse_button_down(MouseButton::Button1) {
            tracing::info!(?ray, "ray");
            // let _scope = TimedScope::new(|elapsed| eprintln!("Ray casting took {:.3?}", elapsed));

            // Perform a ray cast with tractor beam example
            for hit in ray.cast(world, &tree, &()).flatten() {
                tracing::info!(?hit, "hit");
                let query = (effector().as_mut(), velocity(), position());

                let mut query = world.entity(hit.id)?.query(&query);
                // let mut query =
                //     world.query_one::<(&mut Effector, &Velocity, &Position)>(hit.entity)?;

                let point = hit.contact.points[0];

                let (effector, vel, pos) = query.get().context("Failed to get query")?;

                // effector.apply_force(hit.contact.normal * -10.0);
                let sideways_movement = project_plane(*vel, ray.dir());
                let sideways_offset = project_plane(point - *pos, ray.dir());
                let centering = sideways_offset * 500.0;

                let dampening = sideways_movement * -50.0;
                let target = ray.origin() + ray.dir() * 5.0;
                let towards = target - point;
                let towards_vel = (ray.dir() * ray.dir().dot(*vel)).dot(towards.normalize());
                let max_vel = (5.0 * towards.length_squared()).max(0.1);

                let towards = towards.normalize() * 50.0 * (max_vel - towards_vel) / max_vel;

                effector.apply_force(dampening + towards + centering, true);

                for (i, p) in hit.contact.points.iter().enumerate() {
                    gizmos.draw(
                        ivy_base::Sphere {
                            origin: *p,
                            radius: 0.05 / (i + 1) as f32,
                        },
                        Color::from_hsla(i as f32 * 30.0, 1.0, 0.5, 1.0),
                    )
                }
            }
        }

        Query::new((
            velocity().as_mut(),
            position().cmp(|v: &Vec3| v.length() > 100.0),
        ))
        .borrow(world)
        .for_each(|(vel, pos)| {
            if vel.dot(*pos) > 0.0 {
                *vel = -*vel * 0.99;
            }
        });

        Query::new((color().as_mut(), bounding_sphere(), world_transform()))
            .borrow(world)
            .for_each(|(color, bounds, transform)| {
                let (scale, _, pos) = transform.to_scale_rotation_translation();
                *color = if camera.visible(pos, bounds.0 * scale.max_element()) {
                    Color::green()
                } else {
                    Color::red()
                };
            });

        // Draw collisions
        gizmos.begin_section("Draw collisions");
        collision_state.get_all().for_each(|(_, _, v)| {
            v.contact.draw_gizmos(&mut gizmos, Color::yellow());
        });

        // drop(q);

        // WithTime::<RelativeOffset>::update(world, dt);

        // move_system(world, &self.input);

        draw_connections(world, &mut gizmos)?;

        Ok(())
    }
}

struct DisplayDebugReport;

fn setup_ui(world: &mut World, assets: &AssetCache, asset_pack: &AssetPack) -> anyhow::Result<()> {
    let canvas = Query::new(entity_ids())
        .with(canvas())
        .borrow(world)
        .first()
        .unwrap();

    let heart: Asset<Image> = assets.load(&ImageInfo {
        texture: "./res/textures/heart.png".into(),
        sampler: SamplerKey::pixelated(),
    });

    let input_field: Asset<Image> = assets.load(&ImageInfo {
        texture: "./res/textures/field.png".into(),
        sampler: SamplerKey::pixelated(),
    });

    let font: Asset<Font> = assets.load(&FontInfo {
        size: 48.0,
        path: "./res/fonts/Lora/Lora-VariableFont_wght.ttf".into(),
        ..Default::default()
    });

    let monospace: Asset<Font> = assets.load(&FontInfo {
        size: 48.0,
        path: "./res/fonts/Roboto_Mono/RobotoMono-VariableFont_wght.ttf".into(),
        ..Default::default()
    });

    let mut builder = EntityBuilder::new();

    builder
        .mount(WidgetBundle {
            rel_offset: vec2(-0.25, -0.5),
            abs_size: vec2(100.0, 100.0),
            aspect: 1.0,
            ..Default::default()
        })
        .mount(ImageBundle {
            image: Some(heart.clone()),
            color: Color::white(),
        })
        .set(ui_pass(), asset_pack.ui_shader.clone())
        .set(interactive(), ())
        .set(child_of(canvas), ())
        .spawn(world);

    // .mount((
    //     assets.ui_shader,
    //     Interactive,
    //     Reactive::new(Color::white(), Color::gray()),
    // ));

    // world.attach_new::<Widget, _>(canvas, builder.build())?;

    builder
        .mount(InputFieldBundle {
            field: InputField::new(|_, _, val| println!("Input: {:?}", val)),
        })
        .mount(WidgetBundle {
            abs_size: vec2(500.0, 50.0),
            rel_offset: vec2(1.0, -1.0),
            abs_offset: vec2(-20.0, 20.0),
            origin: vec2(1.0, 1.0),
            ..Default::default()
        })
        .mount(TextBundle {
            text: Text::new(""),
            font: font.clone(),
            margin: vec2(10.0, 10.0),
            color: Default::default(),
            wrap: Default::default(),
            align: Default::default(),
        })
        .mount(ImageBundle {
            image: Some(input_field),
            ..Default::default()
        })
        .set(text_pass(), asset_pack.text_shader.clone())
        .set(ui_pass(), asset_pack.ui_shader.clone())
        .set(child_of(canvas), ())
        .spawn(world);

    builder
        .mount(WidgetBundle {
            abs_size: vec2(-10.0, -10.0),
            rel_size: vec2(1.0, 1.0),
            ..Default::default()
        })
        .mount(TextBundle {
            font: monospace,
            text: Text::new("Debug"),
            color: Color::white(),
            align: Alignment::new(HorizontalAlign::Left, VerticalAlign::Top),

            wrap: Default::default(),
            margin: Default::default(),
        })
        .set(text_pass(), asset_pack.text_shader.clone())
        .set(child_of(canvas), ())
        .spawn(world);

    // // .add(DisplayDebugReport);

    // world.attach_new::<Widget, _>(canvas, builder.build())?;

    let widget2 = builder
        .mount(WidgetBundle {
            rel_offset: vec2(0.0, -0.5),
            rel_size: vec2(0.2, 0.2),
            aspect: 1.0,
            ..Default::default()
        })
        .mount(ImageBundle {
            image: Some(heart.clone()),
            color: Color::white(),
        })
        .set(ui_pass(), asset_pack.ui_shader.clone())
        .set(child_of(canvas), ())
        .spawn(world);

    // .add(WithTime::<RelativeOffset>::new(Box::new(
    //     |_, offset, elapsed, _| {
    //         offset.x = (elapsed * 0.25).sin();
    //     },
    // )))
    // .add(Visible::Hidden);

    // let widget2 = world.attach_new::<Widget, _>(canvas, builder.build())?;

    let mut builder = EntityBuilder::new();
    builder
        .mount(WidgetBundle {
            abs_size: vec2(-10.0, -10.0),
            rel_size: vec2(1.0, 1.0),
            aspect: 1.0,
            ..Default::default()
        })
        .mount(ImageBundle {
            image: Some(heart.clone()),
            color: Color::white(),
        })
        .set(ui_pass(), asset_pack.ui_shader.clone())
        .set(child_of(canvas), ())
        .spawn(world);

    // world.attach_new::<Widget, _>(widget2, builder.build())?;

    let mut builder = EntityBuilder::new();
    builder
        .mount(WidgetBundle {
            rel_size: vec2(1.0, 1.0),
            ..Default::default()
        })
        .mount(TextBundle {
            text: Text::new("Hello, World!"),
            font: font.clone(),
            color: Color::purple(),
            align: Alignment::new(HorizontalAlign::Center, VerticalAlign::Top),
            wrap: Default::default(),
            margin: Default::default(),
        })
        .set(text_pass(), asset_pack.text_shader.clone())
        .set(child_of(canvas), ())
        .spawn(world);

    let mut builder = EntityBuilder::new();
    builder
        .mount(WidgetBundle {
            rel_size: vec2(0.5, 0.5),
            aspect: 1.0,
            ..Default::default()
        })
        .mount(TextBundle {
            font,
            text: Text::new("Ivy"),
            color: Color::green(),
            align: Alignment::new(HorizontalAlign::Left, VerticalAlign::Bottom),
            wrap: Default::default(),
            margin: Default::default(),
        })
        .set(text_pass(), asset_pack.text_shader.clone())
        .set(child_of(widget2), ())
        .spawn(world);

    // world.attach_new::<Widget, _>(widget2, builder.build())?;

    let mut builder = EntityBuilder::new();
    let satellite = builder
        .mount(WidgetBundle {
            rel_size: vec2(0.4, 0.4),
            aspect: 1.0,
            ..Default::default()
        })
        .mount(ImageBundle {
            image: Some(heart.clone()),
            color: Color::white(),
        })
        .set(ui_pass(), asset_pack.ui_shader.clone())
        .set(child_of(widget2), ())
        .spawn(world);

    let mut builder = EntityBuilder::new();
    builder
        .mount(WidgetBundle {
            abs_size: vec2(50.0, 50.0),
            aspect: 1.0,
            ..Default::default()
        })
        .mount(ImageBundle {
            image: Some(heart.clone()),
            color: Color::white(),
        })
        .set(ui_pass(), asset_pack.ui_shader.clone())
        .set(child_of(satellite), ())
        .spawn(world);

    Ok(())
}

#[derive(Debug, Clone)]
struct DebugReport {
    framerate: f32,
    min_frametime: Duration,
    avg_frametime: Duration,
    max_frametime: Duration,
    elapsed: Duration,
    position: Vec3,
}

impl Default for DebugReport {
    fn default() -> Self {
        Self {
            framerate: 0.0,
            min_frametime: Duration::from_secs(u64::MAX),
            avg_frametime: Duration::from_secs(0),
            max_frametime: Duration::from_secs(u64::MIN),
            elapsed: Duration::from_secs(0),
            position: Default::default(),
        }
    }
}

impl Display for DebugReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:.2?}, {:.2?}, {:.2?}; {:.0?} fps\n{:.2?}\nPosition: {:.2}\n",
            self.min_frametime,
            self.avg_frametime,
            self.max_frametime,
            self.framerate,
            self.elapsed,
            self.position,
        )?;

        Ok(())
    }
}

struct DebugLayer {
    elapsed: Clock,
    last_status: Clock,
    frequency: Duration,

    min: Duration,
    max: Duration,

    framecount: usize,
}

impl DebugLayer {
    fn new(
        _world: &mut World,
        _assets: &mut AssetCache,
        _events: &mut Events,
        frequency: Duration,
    ) -> anyhow::Result<Self> {
        tracing::debug!("Created debug layer");
        Ok(Self {
            elapsed: Clock::new(),
            last_status: Clock::new(),
            frequency,
            min: Duration::from_secs(u64::MAX),
            max: Duration::from_secs(u64::MIN),
            framecount: 0,
        })
    }
}

struct DebugLayerDesc;

impl LayerDesc for DebugLayerDesc {
    type Layer = DebugLayer;

    fn register(self, _: &mut World, _: &AssetCache) -> anyhow::Result<Self::Layer> {
        Ok(DebugLayer {
            elapsed: Clock::new(),
            last_status: Clock::new(),
            frequency: 100.ms(),
            min: Duration::from_secs(u64::MAX),
            max: Duration::from_secs(u64::MIN),
            framecount: 0,
        })
    }
}

impl Layer for DebugLayer {
    fn on_update(
        &mut self,
        world: &mut World,
        _: &mut AssetCache,
        _: &mut Events,
        frametime: Duration,
    ) -> anyhow::Result<()> {
        // let _scope = TimedScope::new(|elapsed| log::trace!("Debug layer took {:.3?}", elapsed));
        self.min = frametime.min(self.min);
        self.max = frametime.max(self.max);

        self.framecount += 1;

        let elapsed = self.last_status.elapsed();

        if elapsed > self.frequency {
            self.last_status.reset();

            let avg = Duration::div_f32(elapsed, self.framecount as f32);

            self.last_status.reset();

            let report = DebugReport {
                framerate: 1.0 / avg.as_secs_f32(),
                min_frametime: self.min,
                avg_frametime: avg,
                max_frametime: self.max,
                elapsed: self.elapsed.elapsed(),
                position: Query::new(position().copied())
                    .with(main_camera())
                    .borrow(world)
                    .first()
                    .context("No main camera")?,
            };

            tracing::debug!("{:?}", report.framerate);

            // Reset
            self.framecount = 0;
            self.min = Duration::from_secs(u64::MAX);
            self.max = Duration::from_secs(u64::MIN);
        }

        Ok(())
    }
}

#![allow(dead_code)]
use std::{fmt::Display, ops::Deref, sync::Arc, time::Duration};

use anyhow::{anyhow, Context};
use collision::{
    util::project_plane, BinaryNode, Collider, CollisionTree, Cube, Object, Ray, Sphere,
};
use flume::Receiver;
use glfw::{CursorMode, Key, MouseButton, WindowEvent};
use graphics::{
    gizmos::GizmoRenderer,
    layer::{WindowLayer, WindowLayerInfo},
};
use hecs::*;
use hecs_hierarchy::Hierarchy;
use ivy::{
    core::*,
    graphics::*,
    input::*,
    rendergraph::*,
    resources::*,
    ui::{constraints::*, *},
    vulkan::*,
    *,
};
use ivy_resources::Resources;
use parking_lot::RwLock;
use physics::{
    components::{AngularMass, Effector, Mass, Resitution, Velocity},
    PhysicsLayer,
};
use postprocessing::pbr::{create_pbr_pipeline, PBRInfo};
use random::rand::SeedableRng;
use random::{rand::rngs::StdRng, Random};
use rendergraph::GraphicsLayer;
use slotmap::SecondaryMap;
use std::fmt::Write;
use ultraviolet::{Rotor3, Vec2, Vec3};
use vulkan::vk::CullModeFlags;

use log::*;

const FRAMES_IN_FLIGHT: usize = 2;

type CollisionNode = BinaryNode<[Object; 16]>;

struct WithTime<T> {
    func: Box<dyn Fn(Entity, &mut T, f32, f32) + Send + Sync>,
    elapsed: f32,
}

impl<T> WithTime<T>
where
    T: Component,
{
    fn new(func: Box<dyn Fn(Entity, &mut T, f32, f32) + Send + Sync>) -> Self {
        Self { func, elapsed: 0.0 }
    }

    fn update(world: &mut World, dt: f32) {
        world
            .query::<(&mut Self, &mut T)>()
            .iter()
            .for_each(|(e, (s, val))| {
                s.elapsed += dt;
                (s.func)(e, val, s.elapsed, dt);
            });
    }
}

struct Mover {
    translate: InputVector,
    rotate: InputVector,
    local: bool,
    speed: f32,
}

impl Mover {
    fn new(translate: InputVector, rotate: InputVector, speed: f32, local: bool) -> Self {
        Self {
            local,
            translate,
            rotate,
            speed,
        }
    }
}

fn move_system(world: &mut World, input: &Input, dt: f32) {
    world
        .query::<(&Mover, &mut Position, &mut Rotation)>()
        .iter()
        .for_each(|(_, (m, p, r))| {
            if m.local {
                *p += Position(r.into_matrix() * m.translate.get(input)) * m.speed * dt;
            } else {
                *p += Position(m.translate.get(input)) * m.speed * dt;
            }

            let rot = m.rotate.get(input) * dt;
            *r = Rotation(**r * Rotor3::from_euler_angles(rot.x, rot.y, rot.z));
        })
}

struct Periodic<T> {
    func: Box<dyn Fn(Entity, &mut T, usize) + Send + Sync>,
    clock: Clock,
    period: Duration,
    count: usize,
}

impl<T> Periodic<T>
where
    T: Component,
{
    fn _new(period: Duration, func: Box<dyn Fn(Entity, &mut T, usize) + Send + Sync>) -> Self {
        Self {
            func,
            period,
            clock: Clock::new(),
            count: 0,
        }
    }

    fn update(world: &mut World) {
        world
            .query::<(&mut Self, &mut T)>()
            .iter()
            .for_each(|(e, (s, val))| {
                if s.clock.elapsed() >= s.period {
                    s.clock.reset();
                    (s.func)(e, val, s.count);
                    s.count += 1;
                }
            });
    }
}

fn main() -> anyhow::Result<()> {
    Logger {
        show_location: true,
        max_level: LevelFilter::Debug,
    }
    .install();

    // Go up three levels
    ivy_base::normalize_dir(3)?;

    let glfw = Arc::new(RwLock::new(glfw::init(glfw::FAIL_ON_ERRORS)?));

    let window = WindowInfo {
        // extent: Some(Extent::new(800, 600)),
        extent: None,

        resizable: false,
        mode: WindowMode::Windowed,
        ..Default::default()
    };

    let ui_info = UILayerInfo {
        unfocus_key: Some(Key::Escape),
    };

    let mut app = App::builder()
        .try_push_layer(|_, r, _| WindowLayer::new(glfw, r, WindowLayerInfo { window }))?
        .push_layer(|w, r, e| {
            (
                UILayer::new(w, r, e, ui_info),
                ReactiveLayer::<Color>::new(w, r, e),
            )
        })
        .try_push_layer(|w, r, e| -> anyhow::Result<_> {
            Ok(FixedTimeStep::new(
                20.ms(),
                (
                    PhysicsLayer::new(
                        w,
                        r,
                        e,
                        CollisionNode::new(0, Position::default(), Cube::uniform(100.0)),
                    )?,
                    LogicLayer::new(w, r, e)?,
                ),
            ))
        })?
        .try_push_layer(|w, r, e| GraphicsLayer::new(w, r, e, FRAMES_IN_FLIGHT))?
        .try_push_layer(|w, r, e| DebugLayer::new(w, r, e, 100.ms()))?
        .build();

    app.run().context("Failed to run application")
}

fn setup_graphics(
    world: &mut World,
    resources: &Resources,
    camera: Entity,
    canvas: Entity,
) -> anyhow::Result<Assets> {
    let window = resources.get_default::<Window>()?;

    let swapchain_info = ivy_vulkan::SwapchainInfo {
        present_mode: vk::PresentModeKHR::MAILBOX,
        image_count: FRAMES_IN_FLIGHT as u32 + 1,
        ..Default::default()
    };

    let context = resources.get_default::<Arc<VulkanContext>>()?;

    let swapchain = resources.insert_default(Swapchain::new(
        context.clone(),
        window.deref(),
        swapchain_info,
    )?)?;

    let swapchain_extent = resources.get(swapchain)?.extent();

    let final_lit = resources.insert(Texture::new(
        context.clone(),
        &TextureInfo {
            extent: swapchain_extent,
            mip_levels: 1,
            usage: ImageUsage::COLOR_ATTACHMENT | ImageUsage::SAMPLED | ImageUsage::TRANSFER_SRC,
            ..Default::default()
        },
    )?)?;

    let mut rendergraph = RenderGraph::new(context.clone(), FRAMES_IN_FLIGHT)?;

    resources.insert(FullscreenRenderer)?;
    resources.insert(GizmoRenderer::new(context.clone())?)?;

    resources.insert(MeshRenderer::new(context.clone(), 16, FRAMES_IN_FLIGHT)?)?;

    let image_renderer =
        resources.insert(ImageRenderer::new(context.clone(), 16, FRAMES_IN_FLIGHT)?)?;

    let text_renderer = resources.insert(TextRenderer::new(
        context.clone(),
        16,
        512,
        FRAMES_IN_FLIGHT,
    )?)?;

    let pbr_nodes = rendergraph.add_nodes(create_pbr_pipeline::<GeometryPass, PostProcessingPass>(
        context.clone(),
        world,
        &resources,
        camera,
        swapchain_extent,
        FRAMES_IN_FLIGHT,
        &[],
        &[AttachmentInfo {
            store_op: StoreOp::STORE,
            load_op: LoadOp::DONT_CARE,
            initial_layout: ImageLayout::UNDEFINED,
            final_layout: ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            resource: final_lit,
        }],
        &[],
        PBRInfo {
            ambient_radience: Vec3::one() * 0.05,
            max_lights: 10,
        },
    )?);

    let gizmo_node = rendergraph.add_node(CameraNode::<GizmoPass, _, _>::new(
        context.clone(),
        resources,
        camera,
        resources.default::<GizmoRenderer>()?,
        &[AttachmentInfo {
            store_op: StoreOp::STORE,
            load_op: LoadOp::LOAD,
            initial_layout: ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            final_layout: ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            resource: final_lit,
        }],
        &[],
        &[world.get::<DepthAttachment>(camera)?.0],
        None,
        &[],
        &[],
        &[],
        FRAMES_IN_FLIGHT,
    )?);

    let ui_node = rendergraph.add_node(CameraNode::<UIPass, _, _>::new(
        context.clone(),
        resources,
        canvas,
        (image_renderer, text_renderer),
        &[AttachmentInfo {
            store_op: StoreOp::STORE,
            load_op: LoadOp::LOAD,
            initial_layout: ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            final_layout: ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            resource: final_lit,
        }],
        &[],
        &[],
        None,
        &[resources.get(text_renderer)?.vertex_buffer()],
        &[],
        &[],
        FRAMES_IN_FLIGHT,
    )?);

    rendergraph.add_node(TextUpdateNode::new(resources, text_renderer)?);

    rendergraph.add_node(SwapchainNode::new(
        context.clone(),
        swapchain,
        final_lit,
        vec![],
        &resources,
    )?);

    // Build renderpasses
    rendergraph.build(resources.fetch()?, swapchain_extent)?;

    // Create pipelines
    let geometry_node = pbr_nodes[0];
    let post_processing_node = pbr_nodes[1];

    let fullscreen_pipeline = Pipeline::new::<()>(
        context.clone(),
        &PipelineInfo {
            vertexshader: "./res/shaders/fullscreen.vert.spv".into(),
            fragmentshader: "./res/shaders/post_processing.frag.spv".into(),
            samples: SampleCountFlags::TYPE_1,
            extent: swapchain_extent,
            cull_mode: CullModeFlags::NONE,
            ..rendergraph.pipeline_info(post_processing_node)?
        },
    )?;

    let gizmo_pipeline = Pipeline::new::<Vertex>(
        context.clone(),
        &PipelineInfo {
            vertexshader: "./res/shaders/gizmos.vert.spv".into(),
            blending: true,
            fragmentshader: "./res/shaders/gizmos.frag.spv".into(),
            samples: SampleCountFlags::TYPE_1,
            extent: swapchain_extent,
            cull_mode: CullModeFlags::NONE,
            ..rendergraph.pipeline_info(gizmo_node)?
        },
    )?;

    // Create a pipeline from the shaders
    let pipeline = Pipeline::new::<Vertex>(
        context.clone(),
        &PipelineInfo {
            vertexshader: "./res/shaders/default.vert.spv".into(),
            fragmentshader: "./res/shaders/default.frag.spv".into(),
            samples: SampleCountFlags::TYPE_1,
            extent: swapchain_extent,
            ..rendergraph.pipeline_info(geometry_node)?
        },
    )?;

    let geometry_pass = resources.insert(GeometryPass(pipeline))?;

    // Insert one default post processing pass
    resources.insert_default(PostProcessingPass(fullscreen_pipeline))?;
    resources.insert_default(GizmoPass(gizmo_pipeline))?;

    let context = resources.get_default::<Arc<VulkanContext>>()?;

    // Create a pipeline from the shaders
    let ui_pipeline = Pipeline::new::<UIVertex>(
        context.clone(),
        &PipelineInfo {
            blending: true,
            vertexshader: "./res/shaders/ui.vert.spv".into(),
            fragmentshader: "./res/shaders/ui.frag.spv".into(),
            samples: SampleCountFlags::TYPE_1,
            extent: swapchain_extent,
            polygon_mode: vk::PolygonMode::FILL,
            cull_mode: vk::CullModeFlags::NONE,
            front_face: vk::FrontFace::CLOCKWISE,
            ..rendergraph.pipeline_info(ui_node)?
        },
    )?;

    // Create a pipeline from the shaders
    let text_pipeline = Pipeline::new::<UIVertex>(
        context.clone(),
        &PipelineInfo {
            blending: true,
            vertexshader: "./res/shaders/text.vert.spv".into(),
            fragmentshader: "./res/shaders/text.frag.spv".into(),
            samples: SampleCountFlags::TYPE_1,
            extent: swapchain_extent,
            polygon_mode: vk::PolygonMode::FILL,
            cull_mode: vk::CullModeFlags::NONE,
            front_face: vk::FrontFace::CLOCKWISE,
            ..rendergraph.pipeline_info(ui_node)?
        },
    )?;

    let ui_pass = resources.insert(UIPass(ui_pipeline))?;
    let text_pass = resources.insert(UIPass(text_pipeline))?;

    setup_ui(world, resources, ui_pass, text_pass)?;
    resources.insert_default(rendergraph)?;

    Ok(Assets {
        geometry_pass,
        text_pass,
        ui_pass,
    })
}

fn setup_objects(
    world: &mut World,
    resources: &Resources,
    assets: &Assets,
    camera: Entity,
) -> anyhow::Result<Entities> {
    resources.insert(Gizmos::default())?;

    let document: Handle<Document> = resources
        .load("./res/models/cube.gltf")
        .context("Failed to load cube model")??;

    let cube_mesh = resources.get(document)?.mesh(0);

    let document: Handle<Document> = resources
        .load("./res/models/sphere.gltf")
        .context("Failed to load sphere model")??;

    let _sphere_mesh = resources.get(document)?.mesh(0);

    let material: Handle<Material> = resources.load(MaterialInfo {
        albedo: "./res/textures/metal.png".into(),
        roughness: 0.05,
        metallic: 0.9,
        ..Default::default()
    })??;

    world.spawn((
        Position(Vec3::new(0.0, 5.0, 5.0)),
        PointLight::new(1.0, Vec3::new(1.0, 1.0, 0.7) * 5000.0),
    ));

    world.spawn((
        Position(Vec3::new(7.0, 0.0, 0.0)),
        PointLight::new(0.4, Vec3::new(0.0, 0.0, 500.0)),
    ));

    world.spawn((
        Collider::new(Sphere::new(1.0)),
        Color::rgb(1.0, 1.0, 1.0),
        Mass(20.0),
        Velocity::default(),
        Position::new(0.0, 0.6, -1.2),
        Scale::uniform(1.0),
        // Rotation::euler_angles(0.0, 1.0, 1.0),
        Mover::new(
            InputVector {
                x: InputAxis::keyboard(Key::L, Key::H),
                y: InputAxis::keyboard(Key::K, Key::J),
                z: InputAxis::keyboard(Key::I, Key::O),
            },
            InputVector {
                x: InputAxis::none(),
                y: InputAxis::keyboard(Key::Down, Key::Up),
                z: InputAxis::keyboard(Key::Left, Key::Right),
            },
            1.0,
            false,
        ),
        _sphere_mesh,
        material,
        assets.geometry_pass,
    ));

    let mut rng = StdRng::seed_from_u64(43);

    const COUNT: usize = 64;

    world
        .spawn_batch((0..COUNT).map(|_| {
            let pos = Position::rand_uniform(&mut rng) * 10.0;
            let vel = Velocity::rand_uniform(&mut rng);

            (
                AngularMass(5.0),
                Collider::new(Cube::uniform(1.0)),
                Color::rgb(1.0, 1.0, 1.0),
                Mass(10.0),
                pos,
                vel,
                Resitution(0.5),
                Scale::uniform(0.5),
                material,
                assets.geometry_pass,
                cube_mesh,
            )
        }))
        .for_each(|_| {});

    Ok(Entities { camera })
}

struct Assets {
    geometry_pass: Handle<GeometryPass>,
    text_pass: Handle<UIPass>,
    ui_pass: Handle<UIPass>,
}

struct Entities {
    camera: Entity,
}

struct LogicLayer {
    input: Input,

    camera_euler: Vec3,

    cursor_mode: CursorMode,

    window_events: Receiver<WindowEvent>,
    assets: Assets,
    entities: Entities,
}

impl LogicLayer {
    pub fn new(
        world: &mut World,
        resources: &mut Resources,
        events: &mut Events,
    ) -> anyhow::Result<Self> {
        let window = resources.get_default::<Window>()?;

        let input = Input::new(window, events);

        let input_vec = InputVector::new(
            InputAxis::keyboard(Key::D, Key::A),
            InputAxis::keyboard(Key::Space, Key::LeftControl),
            InputAxis::keyboard(Key::S, Key::W),
        );

        let camera = world.spawn((
            Camera::perspective(1.0, input.window_extent().aspect(), 0.1, 100.0),
            Mover::new(input_vec, Default::default(), 5.0, true),
            MainCamera,
            Position(Vec3::new(0.0, 0.0, 5.0)),
            Rotation(Rotor3::identity()),
        ));

        let canvas = world.spawn((
            Canvas,
            Size2D(input.window_extent().into()),
            Position2D::new(0.0, 0.0),
            Camera::default(),
        ));

        let assets =
            setup_graphics(world, resources, camera, canvas).context("Failed to setup graphics")?;

        let entities = setup_objects(world, resources, &assets, camera)?;

        let (tx, window_events) = flume::unbounded();
        events.subscribe(tx);

        Ok(Self {
            input,
            camera_euler: Vec3::zero(),
            entities,
            assets,
            window_events,
            cursor_mode: CursorMode::Normal,
        })
    }

    pub fn handle_events(
        &mut self,
        world: &mut World,
        _resources: &Resources,
    ) -> anyhow::Result<()> {
        // let window = resources.get_default_mut::<Window>()?;

        for event in self.window_events.try_iter() {
            match event {
                WindowEvent::Scroll(_, scroll) => {
                    let mut mover = world.get_mut::<Mover>(self.entities.camera)?;
                    mover.speed = (mover.speed + scroll as f32 * 0.2).clamp(0.1, 20.0);
                }
                _ => {}
            }
        }
        Ok(())
    }
}

impl Layer for LogicLayer {
    fn on_update(
        &mut self,
        world: &mut World,
        resources: &mut Resources,
        _events: &mut Events,
        frame_time: Duration,
    ) -> anyhow::Result<()> {
        let _scope = TimedScope::new(|elapsed| log::trace!("Logic layer took {:.3?}", elapsed));

        self.handle_events(world, resources)
            .context("Failed to handle events")?;

        self.input.handle_events();

        let dt = frame_time.secs();

        let (_e, (camera, camera_pos, camera_rot)) = world
            .query_mut::<(&Camera, &Position, &mut Rotation)>()
            .into_iter()
            .next()
            .unwrap();

        let mut window = resources.get_default_mut::<Window>()?;

        //  Only move camera if right mouse button is held
        if self.input.mouse_button(MouseButton::Button2) {
            window.set_cursor_mode(CursorMode::Disabled);
            let mouse_movement = self.input.cursor_movement() / window.extent().as_vec();

            self.camera_euler += mouse_movement.xyz();
        } else {
            window.set_cursor_mode(CursorMode::Normal);
        }

        *camera_rot = Rotor3::from_euler_angles(
            self.camera_euler.z,
            self.camera_euler.y,
            -self.camera_euler.x,
        )
        .into();

        // Calculate cursor to world ray
        let cursor_pos = self.input.normalized_cursor_pos();

        let dir = camera.to_world_ray(*cursor_pos);

        let ray = Ray::new(*camera_pos, dir);
        let mut gizmos = resources.get_default_mut::<Gizmos>()?;

        let tree = resources.get_default::<CollisionTree<CollisionNode>>()?;

        gizmos.begin_section("ray casting");
        if self.input.mouse_button(MouseButton::Button1) {
            let _scope = TimedScope::new(|elapsed| eprintln!("Ray casting took {:.3?}", elapsed));

            // Perform a ray cast with tractor beam example
            for hit in ray.cast(world, &tree).flatten() {
                let mut query =
                    world.query_one::<(&mut Effector, &Velocity, &Position)>(hit.entity)?;

                let point = hit.contact.points[0];

                let (effector, vel, pos) = query.get().context("Failed to query hit entity")?;

                // effector.apply_force(hit.contact.normal * -10.0);
                let sideways_movement = project_plane(**vel, ray.dir());
                let sideways_offset = project_plane(point - **pos, ray.dir());
                let centering = sideways_offset * 500.0;

                let dampening = sideways_movement * -50.0;
                let target = *ray.origin() + ray.dir() * 5.0;
                let towards = target - point;
                let towards_vel = (ray.dir() * ray.dir().dot(**vel)).dot(towards.normalized());
                let max_vel = (5.0 * towards.mag_sq()).max(0.1);

                let towards = towards.normalized() * 50.0 * (max_vel - towards_vel) / max_vel;

                effector.apply_force(dampening + towards + centering);

                for (i, p) in hit.contact.points.iter().enumerate() {
                    gizmos.push(Gizmo::Sphere {
                        origin: *p,
                        color: Color::hsl(i as f32 * 30.0, 1.0, 0.5),
                        radius: 0.05 / (i + 1) as f32,
                    })
                }
            }
        }

        WithTime::<RelativeOffset>::update(world, dt);
        Periodic::<Text>::update(world);

        move_system(world, &self.input, dt);

        {
            // TODO timed_scope!
            let _scope =
                TimedScope::new(|elapsed| log::trace!("--Graphics updating took {:.3?}", elapsed));

            graphics::systems::satisfy_objects(world);
            graphics::systems::update_view_matrices(world);
        }

        {
            let _scope = TimedScope::new(|elapsed| log::trace!("--UI took {:.3?}", elapsed));

            let canvas = world
                .query::<(&Canvas, &Camera)>()
                .iter()
                .next()
                .ok_or(anyhow!("Missing canvas"))?
                .0;

            ui::systems::statisfy_widgets(world);
            ui::systems::update_canvas(world, canvas)?;
            ui::systems::update(world)?;
        }

        Ok(())
    }
}

new_shaderpass! {
    pub struct GeometryPass;
    pub struct WireframePass;
    pub struct UIPass;
    pub struct GizmoPass;
    pub struct PostProcessingPass;
}

struct DisplayDebugReport;

fn setup_ui(
    world: &mut World,
    resources: &Resources,
    ui_pass: Handle<UIPass>,
    text_pass: Handle<UIPass>,
) -> anyhow::Result<()> {
    let canvas = world
        .query::<&Canvas>()
        .iter()
        .next()
        .ok_or(anyhow!("Missing canvas"))?
        .0;

    let heart: Handle<Image> = resources.load(ImageInfo {
        texture: "./res/textures/heart.png".into(),
        sampler: SamplerInfo::default(),
    })??;

    let input_field: Handle<Image> = resources.load(ImageInfo {
        texture: "./res/textures/field.png".into(),
        sampler: SamplerInfo::default(),
    })??;

    let font: Handle<Font> = resources.load((
        FontInfo {
            size: 64.0,
            ..Default::default()
        },
        "./res/fonts/Lora/Lora-VariableFont_wght.ttf".into(),
    ))??;

    let monospace: Handle<Font> = resources.load((
        FontInfo {
            size: 32.0,
            ..Default::default()
        },
        "./res/fonts/Roboto_Mono/RobotoMono-VariableFont_wght.ttf".into(),
    ))??;

    world.attach_new::<Widget, _>(
        canvas,
        (
            Widget,
            heart,
            ui_pass,
            RelativeOffset::new(-0.25, -0.5),
            AbsoluteSize::new(100.0, 100.0),
            Interactive,
            Reactive {
                unfocused: Color::white(),
                focused: Color::gray(),
            },
            Aspect(1.0),
        ),
    )?;

    InputField::spawn(
        world,
        canvas,
        InputFieldInfo {
            text: Text::new("Sample"),
            text_pass,
            image_pass: ui_pass,
            font,
            reactive: Reactive::new(Color::white(), Color::gray()),
            background: input_field,
            size: AbsoluteSize::new(512.0, 64.0),
            offset: RelativeOffset::new(0.8, 0.8),
            text_padding: Vec2::new(10.0, 10.0),
        },
    )?;

    let widget2 = world.attach_new::<Widget, _>(
        canvas,
        (
            Widget,
            heart,
            ui_pass,
            WithTime::<RelativeOffset>::new(Box::new(|_, offset, elapsed, _| {
                offset.x = (elapsed * 0.25).sin();
            })),
            RelativeOffset::new(0.0, -0.5),
            RelativeSize::new(0.2, 0.2),
            Aspect(1.0),
        ),
    )?;

    world.attach_new::<Widget, _>(
        widget2,
        (Widget, ui_pass, OffsetSize::new(-10.0, -10.0), heart),
    )?;

    world.attach_new::<Widget, _>(
        widget2,
        (
            Widget,
            font,
            Text::new("Hello, World!"),
            Color::black(),
            TextAlignment {
                horizontal: HorizontalAlign::Center,
                vertical: VerticalAlign::Middle,
            },
            WrapStyle::Word,
            RelativeSize::new(1.0, 1.0),
            text_pass,
            AbsoluteOffset::new(0.0, 0.0),
        ),
    )?;

    world.attach_new::<Widget, _>(
        canvas,
        (
            Widget,
            font,
            Text::new("Ivy"),
            TextAlignment::new(HorizontalAlign::Left, VerticalAlign::Bottom),
            text_pass,
            RelativeOffset::new(0.0, 0.0),
            RelativeSize::new(0.5, 0.5),
            Aspect(1.0),
            Color::green(),
        ),
    )?;

    world.attach_new::<Widget, _>(
        canvas,
        (
            Widget,
            monospace,
            text_pass,
            DisplayDebugReport,
            Text::new(""),
            TextAlignment::new(HorizontalAlign::Left, VerticalAlign::Top),
            RelativeOffset::new(0.0, 0.0),
            OffsetSize::new(-10.0, -10.0),
        ),
    )?;

    let satellite = world.attach_new::<Widget, _>(
        widget2,
        (
            Widget,
            heart,
            ui_pass,
            WithTime::<RelativeOffset>::new(Box::new(|_, offset, elapsed, _| {
                *offset = RelativeOffset::new((elapsed).cos() * 4.0, elapsed.sin() * 2.0) * 0.5
            })),
            RelativeOffset::default(),
            RelativeSize::new(0.4, 0.4),
            Aspect(1.0),
        ),
    )?;

    world.attach_new::<Widget, _>(
        satellite,
        (
            Widget,
            heart,
            ui_pass,
            WithTime::<RelativeOffset>::new(Box::new(|_, offset, elapsed, _| {
                *offset = RelativeOffset::new(-(elapsed * 5.0).cos(), -(elapsed * 5.0).sin()) * 0.5
            })),
            RelativeOffset::default(),
            AbsoluteSize::new(50.0, 50.0),
            Aspect(1.0),
        ),
    )?;

    Ok(())
}

#[derive(Debug, Clone)]
struct DebugReport<'a> {
    framerate: f32,
    min_frametime: Duration,
    avg_frametime: Duration,
    max_frametime: Duration,
    elapsed: Duration,
    position: Position,
    execution_times: Option<&'a SecondaryMap<NodeIndex, (&'static str, Duration)>>,
}

impl<'a> Default for DebugReport<'a> {
    fn default() -> Self {
        Self {
            framerate: 0.0,
            min_frametime: Duration::from_secs(u64::MAX),
            avg_frametime: Duration::from_secs(0),
            max_frametime: Duration::from_secs(u64::MIN),
            elapsed: Duration::from_secs(0),
            position: Default::default(),
            execution_times: None,
        }
    }
}

impl<'a> Display for DebugReport<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:.2?}, {:.2?}, {:.2?}; {:.0?} fps\n{:.2?}\n{:#.2?}\n",
            self.min_frametime,
            self.avg_frametime,
            self.max_frametime,
            self.framerate,
            self.elapsed,
            self.position,
        )?;
        self.execution_times
            .map(|val| {
                val.iter()
                    .try_for_each(|(_, val)| write!(f, "{:?}: {}ms\n", val.0, val.1.ms()))
            })
            .transpose()?;

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
        _resources: &Resources,
        _events: &mut Events,
        frequency: Duration,
    ) -> anyhow::Result<Self> {
        log::debug!("Created debug layer");
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

impl Layer for DebugLayer {
    fn on_update(
        &mut self,
        world: &mut World,
        resources: &mut Resources,
        _: &mut Events,
        frametime: Duration,
    ) -> anyhow::Result<()> {
        let _scope = TimedScope::new(|elapsed| log::trace!("Debug layer took {:.3?}", elapsed));
        self.min = frametime.min(self.min);
        self.max = frametime.max(self.max);

        self.framecount += 1;

        let elapsed = self.last_status.elapsed();

        if elapsed > self.frequency {
            self.last_status.reset();

            let avg = Duration::div_f32(elapsed, self.framecount as f32);

            self.last_status.reset();

            let rendergraph = resources.get_default::<RenderGraph>()?;

            let report = DebugReport {
                framerate: 1.0 / avg.secs(),
                min_frametime: self.min,
                avg_frametime: avg,
                max_frametime: self.max,
                elapsed: self.elapsed.elapsed(),
                position: world
                    .query_mut::<(&Position, &MainCamera)>()
                    .into_iter()
                    .next()
                    .map(|(_, (p, _))| *p)
                    .unwrap_or_default(),

                execution_times: Some(rendergraph.execution_times()),
            };

            world
                .query_mut::<(&mut Text, &DisplayDebugReport)>()
                .into_iter()
                .for_each(|(_, (text, _))| {
                    let val = text.val_mut();
                    let val = val.to_mut();

                    val.clear();

                    write!(val, "{}", &report).expect("Failed to write into string");
                });

            log::debug!("{:?}", report.framerate);

            // Reset
            self.framecount = 0;
            self.min = Duration::from_secs(u64::MAX);
            self.max = Duration::from_secs(u64::MIN);
        }

        Ok(())
    }
}

#![allow(dead_code)]
use std::{fmt::Display, sync::Arc, time::Duration};

use anyhow::{anyhow, Context};
use collision::{
    util::project_plane, BinaryNode, Collider, CollisionObject, CollisionTree, Cube, Ray, Sphere,
};
use flume::Receiver;
use glfw::{CursorMode, Key, MouseButton, WindowEvent};
use graphics::{
    gizmos::GizmoRenderer,
    layer::{WindowLayer, WindowLayerInfo},
};
use hecs::*;
use hecs_hierarchy::*;
use ivy::{
    base::*,
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
    bundles::*,
    components::{AngularMass, AngularVelocity, Mass, Velocity},
    connections::{
        draw_connections, Connection, ConnectionBundle, ConnectionKind, PositionOffset,
        RotationOffset,
    },
    Effector, PhysicsLayer, PhysicsLayerInfo,
};
use postprocessing::pbr::{create_pbr_pipeline, PBRInfo};
use random::rand::SeedableRng;
use random::{rand::rngs::StdRng, Random};
use rendergraph::GraphicsLayer;
use slotmap::SecondaryMap;
use std::fmt::Write;
use ultraviolet::{Rotor3, Vec2, Vec3};
use vulkan::vk::{CullModeFlags, PresentModeKHR};

use log::*;

const FRAMES_IN_FLIGHT: usize = 2;

type CollisionNode = BinaryNode<[CollisionObject; 8]>;

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

fn move_system(world: &mut World, input: &Input) {
    world
        .query::<(&Mover, &mut Velocity, &mut AngularVelocity, &Rotation)>()
        .iter()
        .for_each(|(_, (m, v, a, r))| {
            let movement = m.translate.get(&input);
            if m.local {
                *v = Velocity(r.into_matrix() * movement) * m.speed;
            } else {
                *v = Velocity(movement) * m.speed;
            }

            let ang = m.rotate.get(&input);
            *a = ang.into();
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
        extent: None,
        resizable: false,
        mode: WindowMode::Fullscreen,
        ..Default::default()
    };

    let swapchain = SwapchainInfo {
        present_mode: PresentModeKHR::IMMEDIATE,
        image_count: FRAMES_IN_FLIGHT as u32 + 1,
        ..Default::default()
    };

    let mut app = App::builder()
        .try_push_layer(|_, r, _| WindowLayer::new(glfw, r, WindowLayerInfo { window, swapchain }))?
        .push_layer(|w, r, e| (UILayer::new(w, r, e), ReactiveLayer::<Color>::new(w, r, e)))
        .try_push_layer(|w, r, e| -> anyhow::Result<_> {
            Ok(FixedTimeStep::new(
                20.ms(),
                (
                    PhysicsLayer::new(
                        w,
                        r,
                        e,
                        PhysicsLayerInfo {
                            tree_root: CollisionNode::new(
                                0,
                                Position::default(),
                                Cube::uniform(100.0),
                            ),
                            gravity: Gravity::default(),
                        },
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

pub struct RenderGraphNodes {
    pub geometry: NodeIndex,
    pub gizmo: NodeIndex,
    pub ui: NodeIndex,
    pub postprocessing: NodeIndex,
}

impl RenderGraphNodes {
    pub fn new(
        world: &mut World,
        main_camera: Entity,
        canvas: Entity,
        resources: &Resources,
    ) -> anyhow::Result<Self> {
        let context = resources.get_default::<Arc<VulkanContext>>()?;

        let mut rendergraph = RenderGraph::new(context.clone(), FRAMES_IN_FLIGHT)
            .context("Failed to create rendergraph")?;

        let swapchain = resources.get_default::<Swapchain>()?;

        let extent = swapchain.extent();

        let final_lit = resources.insert(Texture::new(
            context.clone(),
            &TextureInfo {
                extent,
                mip_levels: 1,
                usage: ImageUsage::COLOR_ATTACHMENT
                    | ImageUsage::SAMPLED
                    | ImageUsage::TRANSFER_SRC,
                ..Default::default()
            },
        )?)?;

        let image_renderer = resources.default::<ImageRenderer>()?;
        let text_renderer = resources.default::<TextRenderer>()?;

        let pbr_nodes =
            rendergraph.add_nodes(create_pbr_pipeline::<GeometryPass, PostProcessingPass, _>(
                context.clone(),
                world,
                &resources,
                main_camera,
                resources.default::<MeshRenderer>()?,
                extent,
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
        let geometry = pbr_nodes[0];
        let postprocessing = pbr_nodes[1];

        let gizmo = rendergraph.add_node(CameraNode::<GizmoPass, _>::new(
            "Gizmos Node",
            context.clone(),
            resources,
            main_camera,
            resources.default::<GizmoRenderer>()?,
            &[AttachmentInfo {
                store_op: StoreOp::STORE,
                load_op: LoadOp::LOAD,
                initial_layout: ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                final_layout: ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                resource: final_lit,
            }],
            &[],
            &[world.get::<DepthAttachment>(main_camera)?.0],
            None,
            &[],
            &[],
            &[],
            FRAMES_IN_FLIGHT,
        )?);

        let ui = rendergraph.add_node(CameraNode::<UIPass, _>::new(
            "UI Node",
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
            resources.default()?,
            final_lit,
            vec![],
            &resources,
        )?);

        // Build renderpasses
        rendergraph.build(resources.fetch()?, extent)?;

        resources.insert_default(rendergraph)?;

        Ok(Self {
            geometry,
            gizmo,
            ui,
            postprocessing,
        })
    }
}
fn setup_graphics(
    world: &mut World,
    resources: &Resources,
    camera: Entity,
    canvas: Entity,
) -> anyhow::Result<Assets> {
    let context = resources.get_default::<Arc<VulkanContext>>()?;

    // Setup renderers

    resources.insert(FullscreenRenderer)?;
    resources.insert(GizmoRenderer::new(context.clone())?)?;

    resources.insert(MeshRenderer::new(context.clone(), 16, FRAMES_IN_FLIGHT)?)?;

    resources.insert(ImageRenderer::new(context.clone(), 16, FRAMES_IN_FLIGHT)?)?;

    resources.insert(TextRenderer::new(
        context.clone(),
        16,
        512,
        FRAMES_IN_FLIGHT,
    )?)?;

    let nodes = RenderGraphNodes::new(world, camera, canvas, resources)?;

    let rendergraph = resources.get_default::<RenderGraph>()?;

    let postprocessing_pipeline = Pipeline::new::<()>(
        context.clone(),
        &PipelineInfo {
            vertexshader: "./res/shaders/fullscreen.vert.spv".into(),
            fragmentshader: "./res/shaders/post_processing.frag.spv".into(),
            cull_mode: CullModeFlags::NONE,
            ..rendergraph.pipeline_info(nodes.postprocessing)?
        },
    )?;

    let gizmo_pipeline = Pipeline::new::<Vertex>(
        context.clone(),
        &PipelineInfo {
            vertexshader: "./res/shaders/gizmos.vert.spv".into(),
            blending: true,
            fragmentshader: "./res/shaders/gizmos.frag.spv".into(),
            cull_mode: CullModeFlags::NONE,
            ..rendergraph.pipeline_info(nodes.gizmo)?
        },
    )?;

    // Create a pipeline from the shaders
    let pipeline = Pipeline::new::<Vertex>(
        context.clone(),
        &PipelineInfo {
            vertexshader: "./res/shaders/default.vert.spv".into(),
            fragmentshader: "./res/shaders/default.frag.spv".into(),
            samples: SampleCountFlags::TYPE_1,
            ..rendergraph.pipeline_info(nodes.geometry)?
        },
    )?;

    let geometry_pass = resources.insert(GeometryPass(pipeline))?;

    // Insert one default post processing pass
    resources.insert_default(PostProcessingPass(postprocessing_pipeline))?;
    resources.insert_default(GizmoPass(gizmo_pipeline))?;

    let context = resources.get_default::<Arc<VulkanContext>>()?;

    // Create a pipeline from the shaders
    let ui_pipeline = Pipeline::new::<UIVertex>(
        context.clone(),
        &PipelineInfo {
            blending: true,
            vertexshader: "./res/shaders/ui.vert.spv".into(),
            fragmentshader: "./res/shaders/ui.frag.spv".into(),
            ..rendergraph.pipeline_info(nodes.ui)?
        },
    )?;

    // Create a pipeline from the shaders
    let text_pipeline = Pipeline::new::<UIVertex>(
        context.clone(),
        &PipelineInfo {
            blending: true,
            vertexshader: "./res/shaders/text.vert.spv".into(),
            fragmentshader: "./res/shaders/text.frag.spv".into(),
            ..rendergraph.pipeline_info(nodes.ui)?
        },
    )?;

    let ui_pass = resources.insert(UIPass(ui_pipeline))?;
    let text_pass = resources.insert(UIPass(text_pipeline))?;

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
    canvas: Entity,
) -> anyhow::Result<Entities> {
    let _scope = TimedScope::new(|elapsed| eprintln!("Object setup took {:.3?}", elapsed));
    resources.insert(Gizmos::default())?;

    let cube_document: Handle<Document> = resources
        .load("./res/models/cube.glb")
        .context("Failed to load cube model")??;

    let cube_mesh = resources.get(cube_document)?.mesh(0);
    let material = resources.load::<Material, _, _, _>(MaterialInfo {
        albedo: "./res/textures/metal.png".into(),
        sampler: SamplerInfo::default(),
        roughness: 0.1,
        metallic: 1.0,
    })??;

    let sphere_document: Handle<Document> = resources
        .load("./res/models/sphere.gltf")
        .context("Failed to load sphere model")??;

    let sphere_mesh = resources.get(sphere_document)?.mesh(0);

    let mut builder = EntityBuilder::new();

    // resources
    //     .get(cube_document)?
    //     .build_node_by_name("Metal", &mut builder)?
    //     .add_bundle(TransformBundle {
    //         pos: Position::new(0.0, 3.0, 0.0),
    //         rot: Rotation::default(),
    //         scale: Scale::uniform(0.25),
    //     })
    //     .add(assets.geometry_pass);

    world.spawn(builder.build());

    world.spawn((
        Position(Vec3::new(0.0, 5.0, 5.0)),
        PointLight::new(1.0, Vec3::new(1.0, 1.0, 0.7) * 5000.0),
    ));

    let mut builder = EntityBuilder::new();
    builder
        .add_bundle(RbColliderBundle {
            mass: Mass(50.0),
            collider: Collider::new(Sphere::new(1.0)),
            ..Default::default()
        })
        .add_bundle(ObjectBundle {
            pos: Position::new(0.0, 0.6, -1.2),
            scale: Scale::uniform(0.5),
            pass: assets.geometry_pass,
            mesh: sphere_mesh,
            material,
            color: Color::red(),
            ..Default::default()
        });
    // .add(Static);

    let sphere = world.spawn(builder.build());

    let mut builder = EntityBuilder::new();
    builder
        .add_bundle(TransformBundle {
            scale: Scale::uniform(0.5),
            pos: Position::new(0.0, 4.0, 0.0),
            ..Default::default()
        })
        .add_bundle(RbBundle {
            mass: Mass(50.0),
            ..Default::default()
        })
        .add_bundle(ConnectionBundle::new(
            ConnectionKind::spring(100.0, 50.0),
            PositionOffset::new(0.0, 4.0, 0.0),
            RotationOffset::default(),
        ))
        .add_bundle((PointLight::new(0.2, Vec3::new(0.0, 0.0, 5000.0)),));

    let light = world.attach_new::<Connection, _>(sphere, builder.build())?;

    let mut builder = EntityBuilder::new();
    builder
        .add_bundle(ObjectBundle {
            scale: Scale::uniform(0.25),
            mesh: cube_mesh,
            pass: assets.geometry_pass,
            ..Default::default()
        })
        .add_bundle(RbColliderBundle {
            mass: Mass(10.0),
            collider: Collider::new(Cube::uniform(1.0)),
            ..Default::default()
        })
        .add_bundle(ConnectionBundle::new(
            ConnectionKind::spring(10.0, 3.0),
            PositionOffset::new(2.0, 1.0, 0.0),
            RotationOffset::default(),
        ));

    world.attach_new::<Connection, _>(light, builder.build())?;

    let mut builder = EntityBuilder::new();

    builder
        .add_bundle(ObjectBundle {
            scale: Scale::uniform(0.25),
            mesh: sphere_mesh,
            pass: assets.geometry_pass,
            material,
            ..Default::default()
        })
        .add_bundle(RbColliderBundle {
            collider: Collider::new(Sphere::new(1.0)),
            mass: Mass(10.0),
            ..Default::default()
        })
        .add_bundle(ConnectionBundle::new(
            ConnectionKind::Rigid,
            PositionOffset::new(-1.0, 0.0, 2.0),
            Default::default(),
        ));

    world.attach_new::<Connection, _>(light, builder.build())?;
    let mut rng = StdRng::seed_from_u64(42);
    // world.spawn(builder.build());

    const COUNT: usize = 64;

    (0..COUNT).for_each(|_| {
        let pos = Position::rand_uniform(&mut rng) * 10.0;
        let vel = Velocity::rand_uniform(&mut rng);
        builder
            .add_bundle(ObjectBundle {
                mesh: cube_mesh,
                pass: assets.geometry_pass,
                scale: Scale::uniform(0.5),
                pos,
                ..Default::default()
            })
            .add_bundle(RbColliderBundle {
                collider: Collider::new(Cube::uniform(1.0)),
                vel,
                mass: Mass(20.0),
                ang_mass: AngularMass(2.0),
                ..Default::default()
            })
            .add_bundle(ConnectionBundle::new(
                ConnectionKind::Rigid,
                PositionOffset::new(-1.0, 0.0, 2.0),
                Default::default(),
            ));

        world.spawn(builder.build());
    });

    Ok(Entities { camera, canvas })
}

struct Assets {
    geometry_pass: Handle<GeometryPass>,
    text_pass: Handle<UIPass>,
    ui_pass: Handle<UIPass>,
}

struct Entities {
    camera: Entity,
    canvas: Entity,
}

struct LogicLayer {
    input: Input,

    camera_euler: Vec3,

    cursor_mode: CursorMode,

    window_events: Receiver<WindowEvent>,
    graphics_events: Receiver<GraphicsEvent>,
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
            InputAxis::keyboard(Key::A, Key::D),
            InputAxis::keyboard(Key::Space, Key::LeftControl),
            InputAxis::keyboard(Key::W, Key::S),
        );

        let mut builder = EntityBuilder::new();

        builder
            .add_bundle(TransformBundle {
                pos: Position::new(0.0, 0.0, -7.0),
                ..Default::default()
            })
            .add_bundle(RbBundle::default())
            .add_bundle((
                MainCamera,
                Camera::perspective(1.0, input.window_extent().aspect(), 0.1, 100.0),
                Mover::new(input_vec, Default::default(), 5.0, true),
            ));

        let camera = world.spawn(builder.build());

        let mut builder = EntityBuilder::new();
        builder.add_bundle(CanvasBundle::new(input.window_extent()));

        let canvas = world.spawn(builder.build());

        let assets =
            setup_graphics(world, resources, camera, canvas).context("Failed to setup graphics")?;

        let entities = setup_objects(world, resources, &assets, camera, canvas)?;

        setup_ui(world, resources, &assets)?;

        let (tx, window_events) = flume::unbounded();
        events.subscribe(tx);

        let (tx, graphics_events) = flume::unbounded();
        events.subscribe(tx);

        Ok(Self {
            input,
            camera_euler: Vec3::zero(),
            entities,
            assets,
            window_events,
            graphics_events,
            cursor_mode: CursorMode::Normal,
        })
    }

    pub fn handle_events(
        &mut self,
        world: &mut World,
        resources: &Resources,
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

        for event in self.graphics_events.try_iter() {
            match event {
                GraphicsEvent::SwapchainRecreation => {
                    setup_graphics(world, resources, self.entities.camera, self.entities.canvas)?;
                }
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
        // let _scope = TimedScope::new(|elapsed| log::trace!("Logic layer took {:.3?}", elapsed));

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

        gizmos.begin_section("Ray Casting");
        if self.input.mouse_button(MouseButton::Button1) {
            // let _scope = TimedScope::new(|elapsed| eprintln!("Ray casting took {:.3?}", elapsed));

            // Perform a ray cast with tractor beam example
            for hit in ray.cast(world, &tree).flatten() {
                let mut query =
                    world.query_one::<(&mut Effector, &Velocity, &Position)>(hit.entity)?;

                let point = hit.contact.points[0];

                let (effector, vel, pos) = query.get().context("Failed to query hit entity")?;

                // effector.apply_force(hit.contact.normal * -10.0);
                let sideways_movement = project_plane(**vel, ray.dir());
                let sideways_offset = project_plane(*point - **pos, ray.dir());
                let centering = sideways_offset * 500.0;

                let dampening = sideways_movement * -50.0;
                let target = *ray.origin() + ray.dir() * 5.0;
                let towards = target - *point;
                let towards_vel = (ray.dir() * ray.dir().dot(**vel)).dot(towards.normalized());
                let max_vel = (5.0 * towards.mag_sq()).max(0.1);

                let towards = towards.normalized() * 50.0 * (max_vel - towards_vel) / max_vel;

                effector.apply_force(dampening + towards + centering);

                for (i, p) in hit.contact.points.iter().enumerate() {
                    gizmos.draw(Gizmo::Sphere {
                        origin: *p,
                        color: Color::hsl(i as f32 * 30.0, 1.0, 0.5),
                        radius: 0.05 / (i + 1) as f32,
                    })
                }
            }
        }

        WithTime::<RelativeOffset>::update(world, dt);
        Periodic::<Text>::update(world);

        move_system(world, &self.input);

        graphics::systems::update_view_matrices(world);

        draw_connections(world, &mut gizmos)?;

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

fn setup_ui(world: &mut World, resources: &Resources, assets: &Assets) -> anyhow::Result<()> {
    let canvas = world
        .query::<&Canvas>()
        .iter()
        .next()
        .ok_or(anyhow!("Missing canvas"))?
        .0;

    let heart: Handle<Image> = resources.load(ImageInfo {
        texture: "./res/textures/heart.png".into(),
        sampler: SamplerInfo::pixelated(),
    })??;

    let input_field: Handle<Image> = resources.load(ImageInfo {
        texture: "./res/textures/field.png".into(),
        sampler: SamplerInfo::pixelated(),
    })??;

    let font: Handle<Font> = resources.load(FontInfo {
        size: 48.0,
        path: "./res/fonts/Lora/Lora-VariableFont_wght.ttf".into(),
        ..Default::default()
    })??;

    let monospace: Handle<Font> = resources.load(FontInfo {
        size: 48.0,
        path: "./res/fonts/Roboto_Mono/RobotoMono-VariableFont_wght.ttf".into(),
        ..Default::default()
    })??;

    let mut builder = EntityBuilder::new();
    builder
        .add_bundle(WidgetBundle {
            rel_offset: RelativeOffset::new(-0.25, -0.5),
            abs_size: AbsoluteSize::new(100.0, 100.0),
            aspect: Aspect(1.0),
            ..Default::default()
        })
        .add_bundle(ImageBundle {
            image: heart,
            pass: assets.ui_pass,
            color: Color::white(),
        })
        .add_bundle((Interactive, Reactive::new(Color::white(), Color::gray())));

    world.attach_new::<Widget, _>(canvas, builder.build())?;

    let mut builder = EntityBuilder::new();
    builder
        .add_bundle(WidgetBundle {
            rel_offset: RelativeOffset::new(1.0, -1.0),
            abs_offset: AbsoluteOffset::new(-10.0, 10.0),
            abs_size: AbsoluteSize::new(512.0, 60.0),
            origin: Origin2D::new(1.0, -1.0),
            aspect: Aspect(0.0),
            ..Default::default()
        })
        .add_bundle(ImageBundle {
            image: input_field,
            color: Color::white(),
            pass: assets.ui_pass,
        })
        .add(Reactive::new(Color::white(), Color::gray()));

    InputField::spawn(
        world,
        canvas,
        InputFieldInfo {
            text: TextBundle {
                font,
                pass: assets.text_pass,
                align: Alignment::new(HorizontalAlign::Left, VerticalAlign::Middle),
                ..Default::default()
            },
            field: builder,
            placeholder: "Enter text:".into(),
            text_padding: Vec2::new(10.0, 10.0),
        },
    )?;

    let mut builder = EntityBuilder::new();
    builder
        .add_bundle(WidgetBundle {
            abs_size: AbsoluteSize::new(-10.0, -10.0),
            rel_size: RelativeSize::new(1.0, 1.0),
            ..Default::default()
        })
        .add_bundle(TextBundle {
            font: monospace,
            text: Text::new("Debug"),
            color: Color::white(),
            align: Alignment::new(HorizontalAlign::Left, VerticalAlign::Top),
            pass: assets.text_pass,
            ..Default::default()
        })
        .add(DisplayDebugReport);

    world.attach_new::<Widget, _>(canvas, builder.build())?;

    let mut builder = EntityBuilder::new();
    builder
        .add_bundle(WidgetBundle {
            rel_offset: RelativeOffset::new(0.0, -0.5),
            rel_size: RelativeSize::new(0.2, 0.2),
            aspect: Aspect(1.0),
            ..Default::default()
        })
        .add_bundle(ImageBundle {
            image: heart,
            color: Color::white(),
            pass: assets.ui_pass,
        })
        .add(WithTime::<RelativeOffset>::new(Box::new(
            |_, offset, elapsed, _| {
                offset.x = (elapsed * 0.25).sin();
            },
        )))
        .add(Visible::Hidden);

    let widget2 = world.attach_new::<Widget, _>(canvas, builder.build())?;

    let mut builder = EntityBuilder::new();
    builder
        .add_bundle(WidgetBundle {
            abs_size: AbsoluteSize::new(-10.0, -10.0),
            rel_size: RelativeSize::new(1.0, 1.0),
            aspect: Aspect(1.0),
            ..Default::default()
        })
        .add_bundle(ImageBundle {
            image: heart,
            color: Color::white(),
            pass: assets.ui_pass,
        });

    world.attach_new::<Widget, _>(widget2, builder.build())?;

    let mut builder = EntityBuilder::new();
    builder
        .add_bundle(WidgetBundle {
            rel_size: RelativeSize::new(1.0, 1.0),
            ..Default::default()
        })
        .add_bundle(TextBundle {
            text: Text::new("Hello, World!"),
            font,
            color: Color::purple(),
            align: Alignment::new(HorizontalAlign::Center, VerticalAlign::Top),
            pass: assets.text_pass,
            ..Default::default()
        });

    world.attach_new::<Widget, _>(widget2, builder.build())?;

    let mut builder = EntityBuilder::new();
    builder
        .add_bundle(WidgetBundle {
            rel_size: RelativeSize::new(0.5, 0.5),
            aspect: Aspect(1.0),
            ..Default::default()
        })
        .add_bundle(TextBundle {
            font,

            text: Text::new("Ivy"),
            color: Color::dark_green(),
            align: Alignment::new(HorizontalAlign::Left, VerticalAlign::Bottom),
            pass: assets.text_pass,
            ..Default::default()
        });

    world.attach_new::<Widget, _>(widget2, builder.build())?;

    let mut builder = EntityBuilder::new();
    builder
        .add_bundle(WidgetBundle {
            rel_size: RelativeSize::new(0.4, 0.4),
            aspect: Aspect(1.0),
            ..Default::default()
        })
        .add_bundle(ImageBundle {
            image: heart,
            color: Color::white(),
            pass: assets.ui_pass,
        })
        .add(WithTime::<RelativeOffset>::new(Box::new(
            |_, offset, elapsed, _| {
                *offset = RelativeOffset::new((elapsed).cos() * 4.0, elapsed.sin() * 2.0) * 0.5
            },
        )));

    let satellite = world.attach_new::<Widget, _>(widget2, builder.build())?;

    let mut builder = EntityBuilder::new();
    builder
        .add_bundle(WidgetBundle {
            abs_size: AbsoluteSize::new(50.0, 50.0),
            aspect: Aspect(1.0),
            ..Default::default()
        })
        .add_bundle(ImageBundle {
            image: heart,
            color: Color::white(),
            pass: assets.ui_pass,
        })
        .add(WithTime::<RelativeOffset>::new(Box::new(
            |_, offset, elapsed, _| {
                *offset = RelativeOffset::new(-(elapsed * 5.0).cos(), -(elapsed * 5.0).sin()) * 0.5
            },
        )));

    world.attach_new::<Widget, _>(satellite, builder.build())?;

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
        // let _scope = TimedScope::new(|elapsed| log::trace!("Debug layer took {:.3?}", elapsed));
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

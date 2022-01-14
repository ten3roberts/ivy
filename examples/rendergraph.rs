use base::{BuilderExt, Events};
use glam::Vec3;
use graphics::{
    shaders::{FORWARD_FRAGMENT_SHADER, FORWARD_VERTEX_SHADER},
    NodeBuildInfo,
};
use hecs::{EntityBuilder, World};
use ivy::*;
use physics::PhysicsLayer;
use presets::GeometryPass;
use rendergraph::{AttachmentInfo, CameraNode, CameraNodeInfo, GraphicsLayer, SwapchainNode};
use resources::LoadResource;
use vulkan::{
    context::SharedVulkanContext,
    vk::{ClearValue, PresentModeKHR},
    ClearValueExt, PipelineInfo, SwapchainInfo, TextureInfo,
};
pub struct GameLayer {}

const FRAMES_IN_FLIGHT: usize = 2;
impl GameLayer {
    pub fn new(
        world: &mut World,
        resources: &mut Resources,
        _: &mut Events,
    ) -> anyhow::Result<Self> {
        Self::setup(world, resources)?;
        Ok(Self {})
    }

    pub fn setup(world: &mut World, resources: &mut Resources) -> anyhow::Result<()> {
        let window = resources.get_default::<Window>()?;

        // Create a camera
        let camera = EntityBuilder::new()
            .add_bundle((
                Camera::perspective(1.0, window.aspect(), 0.1, 100.0),
                MainCamera,
            ))
            .add_bundle(TransformBundle {
                pos: Position::new(0.0, 0.0, -3.0),
                ..Default::default()
            })
            .spawn(world);

        let context = resources.get_default::<SharedVulkanContext>()?;

        // Create or get existing rendergraph
        let mut rendergraph = resources
            .default_entry()?
            .or_try_insert_with(|| RenderGraph::new(context.clone(), FRAMES_IN_FLIGHT))?;

        // Create a texture to render into
        let color = resources.insert(Texture::new(
            context.clone(),
            &TextureInfo {
                extent: window.extent(),
                usage: ImageUsage::COLOR_ATTACHMENT
                    | ImageUsage::TRANSFER_SRC
                    | ImageUsage::SAMPLED,
                ..Default::default()
            },
        )?)?;

        // Create a depth buffer to ensure proper ordering
        let depth = resources.insert(Texture::new(
            context.clone(),
            &TextureInfo::depth(window.extent()),
        )?)?;

        // Get or create a mesh renderer
        let renderer = resources
            .default_entry::<MeshRenderer>()?
            .or_try_insert_with(|| MeshRenderer::new(context.clone(), 128, FRAMES_IN_FLIGHT))?
            .handle;

        // Add a node rendering the scene from the camera
        rendergraph.add_node(CameraNode::<GeometryPass, _>::new(
            context.clone(),
            resources,
            camera,
            renderer,
            CameraNodeInfo {
                color_attachments: vec![AttachmentInfo::color(color)],
                depth_attachment: Some(AttachmentInfo::depth_discard(depth)),
                clear_values: vec![
                    ClearValue::color(0.0, 0.0, 0.0, 0.0),
                    ClearValue::depth_stencil(1.0, 0),
                ],
                ..Default::default()
            },
        )?);

        // Add a node to present the texture to the swapchain and window
        rendergraph.add_node(SwapchainNode::new(
            context.clone(),
            &resources,
            resources.default()?,
            color,
        )?);

        rendergraph.build(resources.fetch()?, window.extent())?;

        // Create a simple pipeline
        let pipeline = GeometryPass(PipelineInfo {
            vs: FORWARD_VERTEX_SHADER,
            fs: FORWARD_FRAGMENT_SHADER,
            ..Default::default()
        });

        let pass = resources.insert(pipeline)?;

        let document = Document::load(resources, &"./res/models/monkey.gltf".into())?;

        let mut builder = EntityBuilder::new();
        document
            .find("Suzanne")?
            .build(
                &mut builder,
                &NodeBuildInfo {
                    skinned: false,
                    light_radius: 0.1,
                },
            )
            .add(pass)
            .add_bundle(RbBundle {
                ang_vel: AngularVelocity::new(0.0, 1.0, 0.0),
                ..Default::default()
            })
            .spawn(world);

        Ok(())
    }
}

impl Layer for GameLayer {
    fn on_update(
        &mut self,
        _: &mut hecs::World,
        _: &mut Resources,
        _: &mut base::Events,
        _: std::time::Duration,
    ) -> anyhow::Result<()> {
        Ok(())
    }
}

fn main() -> anyhow::Result<()> {
    let window = WindowLayerInfo {
        swapchain: SwapchainInfo {
            present_mode: PresentModeKHR::FIFO,
            ..Default::default()
        },
        ..Default::default()
    };

    App::builder()
        .try_push_layer(|_, r, _| WindowLayer::new(r, window))?
        .try_push_layer(GameLayer::new)?
        .try_push_layer(|w, r, e| GraphicsLayer::new(w, r, e, FRAMES_IN_FLIGHT))?
        .try_push_layer(|w, r, e| {
            PhysicsLayer::new(
                w,
                r,
                e,
                physics::PhysicsLayerInfo {
                    gravity: Vec3::ZERO.into(),
                    tree_root: (), // Disable collisions
                    debug: false,
                },
            )
        })?
        .build()
        .run()
}

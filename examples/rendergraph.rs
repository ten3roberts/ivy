use std::time::Duration;

use base::Events;
use flax::{Entity, EntityBuilder, World};
use glam::{vec3, Vec3};
use graphics::{
    components::camera,
    shaders::{FORWARD_FRAGMENT_SHADER, FORWARD_VERTEX_SHADER},
    NodeBuildInfo,
};
use ivy_engine::*;
use physics::PhysicsLayer;
use presets::geometry_pass;
use rendergraph::{AttachmentInfo, CameraNode, CameraNodeInfo, GraphicsLayer, SwapchainPresentNode};
use resources::LoadResource;
use vulkan::{
    context::SharedVulkanContext,
    vk::{ClearValue, PresentModeKHR},
    ClearValueExt, PipelineInfo, Shader, SwapchainInfo, TextureInfo,
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
        let camera = Entity::builder()
            .set(
                camera(),
                Camera::perspective(1.0, window.aspect(), 0.1, 1000.0),
            )
            .set_default(main_camera())
            .set(position(), vec3(0.0, 0.0, -3.0))
            .set_default(scale())
            .set_default(rotation())
            .spawn(world);

        // .add_bundle((
        //     Camera::perspective(1.0, window.aspect(), 0.1, 100.0),
        //     MainCamera,
        // ))
        // .add_bundle( {
        //     pos: vec3(0.0, 0.0, -3.0),
        //     ..Default::default()
        // })
        // .spawn(world);

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
        rendergraph.add_node(CameraNode::new(
            context.clone(),
            world,
            resources,
            camera,
            renderer,
            geometry_pass(),
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
        rendergraph.add_node(SwapchainPresentNode::new(
            context.clone(),
            &resources,
            resources.default()?,
            color,
        )?);

        rendergraph.build(resources.fetch()?, window.extent())?;

        // Create a simple shader
        let pass = Shader::new(resources.insert(PipelineInfo {
            vs: FORWARD_VERTEX_SHADER,
            fs: FORWARD_FRAGMENT_SHADER,
            ..Default::default()
        })?);

        let document = Document::load(resources, &"./res/models/monkey.gltf".into())?;

        let mut builder = EntityBuilder::new();

        document
            .find("Suzanne")?
            .mount(
                &mut builder,
                &NodeBuildInfo {
                    skinned: false,
                    animated: false,
                    light_radius: 0.1,
                },
            )
            .set(geometry_pass(), pass);

        RbBundle {
            ang_vel: vec3(0.0, 1.0, 0.0),
            ..Default::default()
        }
        .mount(&mut builder)
        .spawn(world);

        Ok(())
    }
}

impl Layer for GameLayer {
    fn on_update(
        &mut self,
        _: &mut World,
        _: &mut Resources,
        _: &mut Events,
        _: Duration,
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

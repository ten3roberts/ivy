use hecs::{Component, Entity, World};
use ivy_base::Extent;
use ivy_graphics::{DepthAttachment, EnvironmentManager, GpuCameraData, LightManager, Renderer};
use ivy_rendergraph::{AttachmentInfo, CameraNode, CameraNodeInfo, Node};
use ivy_resources::{Handle, Resources, Storage};
use ivy_vulkan::{
    context::SharedVulkanContext, descriptors::MultiDescriptorBindable, shaderpass::ShaderPass,
    vk::ClearValue, ClearValueExt, ImageLayout, LoadOp, StoreOp, Texture,
};

mod attachments;

use crate::node::PostProcessingNode;

use self::attachments::PBRAttachments;

#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PBRInfo<EnvData> {
    pub max_lights: u64,
    pub env_data: EnvData,
}

impl<EnvData: Default> Default for PBRInfo<EnvData> {
    fn default() -> Self {
        Self {
            max_lights: 5,
            env_data: EnvData::default(),
        }
    }
}

/// Installs PBR rendering for the specified camera. Returns a list of nodes suitable for
/// rendergraph insertions. Configures gpu camera data, light management and
/// environment manager and attaches them to the camera.
pub fn create_pbr_pipeline<GeometryPass, PostProcessingPass, EnvData, R>(
    context: SharedVulkanContext,
    world: &mut World,
    resources: &Resources,
    camera: Entity,
    renderer: R,
    extent: Extent,
    frames_in_flight: usize,
    read_attachments: &[Handle<Texture>],
    color_attachments: &[AttachmentInfo],
    bindables: &[&dyn MultiDescriptorBindable],
    info: PBRInfo<EnvData>,
) -> ivy_rendergraph::Result<[Box<dyn Node>; 2]>
where
    GeometryPass: ShaderPass,
    PostProcessingPass: ShaderPass,
    R: Renderer + Storage,
    R::Error: Storage + Into<anyhow::Error>,
    EnvData: Copy + Component,
{
    let pbr_attachments = PBRAttachments::new(context.clone(), resources, extent)?;

    let depth_attachment = DepthAttachment::new(context.clone(), resources, extent)?;

    let camera_node = Box::new(CameraNode::<GeometryPass, R>::new(
        context.clone(),
        resources,
        camera,
        renderer,
        CameraNodeInfo {
            name: "PBR Camera Node",
            color_attachments: pbr_attachments
                .as_slice()
                .iter()
                .map(|resource| AttachmentInfo {
                    store_op: StoreOp::STORE,
                    load_op: LoadOp::CLEAR,
                    initial_layout: ImageLayout::UNDEFINED,
                    final_layout: ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                    resource: *resource,
                    clear_value: ClearValue::color(0.0, 0.0, 0.0, 1.0),
                })
                .collect::<Vec<_>>(),
            depth_attachment: Some(AttachmentInfo {
                store_op: StoreOp::STORE,
                load_op: LoadOp::CLEAR,
                initial_layout: ImageLayout::UNDEFINED,
                final_layout: ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                resource: *depth_attachment,
                clear_value: ClearValue::depth_stencil(1.0, 0),
            }),
            frames_in_flight,
            ..Default::default()
        },
    )?);

    let light_manager = LightManager::new(context.clone(), info.max_lights, frames_in_flight)?;
    let env_manager = EnvironmentManager::new(context.clone(), info.env_data, frames_in_flight)?;
    let camera_data = GpuCameraData::new(context.clone(), frames_in_flight)?;

    let data = [
        camera_data.buffers(),
        light_manager.scene_buffers(),
        light_manager.light_buffers(),
        env_manager.buffers(),
    ];

    let bindables = data
        .iter()
        .map(|val| val as &dyn MultiDescriptorBindable)
        .chain(bindables.into_iter().cloned())
        .collect::<Vec<_>>();

    let input_attachments = [
        pbr_attachments.albedo,
        pbr_attachments.position,
        pbr_attachments.normal,
        pbr_attachments.roughness_metallic,
        *depth_attachment,
    ];

    let post_processing_node = Box::new(PostProcessingNode::<PostProcessingPass>::new(
        context.clone(),
        resources,
        read_attachments,
        &input_attachments,
        &bindables,
        color_attachments,
        frames_in_flight,
    )?);

    // Store data in camera
    world
        .insert(
            camera,
            (
                light_manager,
                camera_data,
                pbr_attachments,
                depth_attachment,
                env_manager,
            ),
        )
        .expect("Entity is valid");

    Ok([camera_node, post_processing_node])
}

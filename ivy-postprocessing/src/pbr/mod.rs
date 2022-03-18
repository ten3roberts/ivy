use hecs::{Component, Entity, World};
use itertools::Itertools;
use ivy_base::Extent;
use ivy_graphics::{
    DepthAttachment, EnvironmentManager, FullscreenRenderer, GpuCameraData, LightRenderer,
    MeshRenderer, Renderer,
};
use ivy_rendergraph::{AttachmentInfo, CameraNode, CameraNodeInfo, Node, TransferNode};
use ivy_resources::{Handle, Resources, Storage};
use ivy_vulkan::{
    context::SharedVulkanContext,
    descriptors::MultiDescriptorBindable,
    shaderpass::ShaderPass,
    vk::{ClearValue, ImageAspectFlags, ShaderStageFlags},
    AddressMode, ClearValueExt, FilterMode, Format, ImageLayout, ImageUsage, LoadOp, Sampler,
    SamplerInfo, StoreOp, Texture, TextureInfo,
};

mod attachments;

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
pub fn create_pbr_pipeline<GeometryPass, PostProcessingPass, TransparentPass, EnvData, R>(
    context: SharedVulkanContext,
    world: &mut World,
    resources: &Resources,
    camera: Entity,
    renderer: R,
    extent: Extent,
    frames_in_flight: usize,
    read_attachments: &[Handle<Texture>],
    color_attachments: &[AttachmentInfo],
    info: PBRInfo<EnvData>,
) -> ivy_rendergraph::Result<[Box<dyn Node>; 6]>
where
    GeometryPass: ShaderPass,
    PostProcessingPass: ShaderPass,
    TransparentPass: ShaderPass,
    R: Renderer + Storage + Clone,
    R::Error: Storage + Into<anyhow::Error>,
    EnvData: Copy + Component,
{
    GpuCameraData::create_gpu_cameras(&context, world, frames_in_flight)?;
    let pbr_attachments = PBRAttachments::new(context.clone(), resources, extent)?;

    let depth_attachment = DepthAttachment::new(context.clone(), resources, extent)?;

    let camera_node = Box::new(CameraNode::<GeometryPass, R>::new(
        context.clone(),
        world,
        resources,
        camera,
        renderer.clone(),
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
                final_layout: ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
                resource: *depth_attachment,
                clear_value: ClearValue::depth_stencil(1.0, 0),
            }),
            frames_in_flight,
            ..Default::default()
        },
    )?);

    let light_renderer = LightRenderer::new(context.clone(), info.max_lights, frames_in_flight)?;
    let env_manager = EnvironmentManager::new(context.clone(), info.env_data, frames_in_flight)?;

    let data = [&env_manager.buffers() as &dyn MultiDescriptorBindable];

    let input_attachments = [
        pbr_attachments.albedo,
        pbr_attachments.position,
        pbr_attachments.normal,
        pbr_attachments.roughness_metallic,
        *depth_attachment,
    ];

    // TODO one image
    let final_lit = resources.insert(Texture::new(
        context.clone(),
        &TextureInfo {
            extent,
            mip_levels: 1,
            usage: ImageUsage::COLOR_ATTACHMENT
                | ImageUsage::INPUT_ATTACHMENT
                | ImageUsage::TRANSFER_SRC,
            ..Default::default()
        },
    )?)?;

    let sampler: Handle<Sampler> = resources.load(SamplerInfo {
        address_mode: AddressMode::CLAMP_TO_EDGE,
        mag_filter: FilterMode::LINEAR,
        min_filter: FilterMode::LINEAR,
        unnormalized_coordinates: false,
        anisotropy: 16,
        mip_levels: 1,
    })??;

    let light_node = Box::new(CameraNode::<(), _>::new(
        context.clone(),
        world,
        resources,
        camera,
        light_renderer,
        CameraNodeInfo {
            name: "Light renderer",
            color_attachments: vec![AttachmentInfo::color(final_lit)],
            input_attachments: input_attachments.to_vec(),
            depth_attachment: None,
            camera_stage: ShaderStageFlags::VERTEX | ShaderStageFlags::FRAGMENT,
            frames_in_flight,
            ..Default::default()
        },
    )?);

    let screenview = resources.insert(Texture::new(
        context.clone(),
        &TextureInfo {
            extent,
            mip_levels: 1,
            usage: ImageUsage::TRANSFER_DST | ImageUsage::SAMPLED,
            ..Default::default()
        },
    )?)?;

    let screenview_d = resources.insert(Texture::new(
        context.clone(),
        &TextureInfo {
            extent,
            mip_levels: 1,
            usage: ImageUsage::TRANSFER_DST
                | ImageUsage::SAMPLED
                | ImageUsage::DEPTH_STENCIL_ATTACHMENT,

            format: Format::D32_SFLOAT,
            ..Default::default()
        },
    )?)?;

    // Copy lit to the attachment to read
    let transfer = Box::new(TransferNode::new(
        final_lit,
        screenview,
        ImageLayout::TRANSFER_SRC_OPTIMAL,
        ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
        ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        ImageAspectFlags::COLOR,
    )?);

    // Copy lit to the attachment to read
    let transfer_depth = Box::new(TransferNode::new(
        *depth_attachment,
        screenview_d,
        ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
        ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
        ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        ImageAspectFlags::DEPTH,
    )?);

    let transparent = Box::new(CameraNode::<TransparentPass, _>::new(
        context.clone(),
        world,
        resources,
        camera,
        resources.default::<MeshRenderer>()?,
        CameraNodeInfo {
            name: "Transparent",
            color_attachments: vec![AttachmentInfo {
                store_op: StoreOp::STORE,
                load_op: LoadOp::LOAD,
                initial_layout: ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                final_layout: ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                resource: final_lit,
                clear_value: ClearValue::color(0.0, 0.0, 0.0, 0.0),
            }],
            depth_attachment: Some(AttachmentInfo {
                resource: *depth_attachment,
                store_op: StoreOp::STORE,
                load_op: LoadOp::LOAD,
                initial_layout: ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
                final_layout: ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
                ..Default::default()
            }),
            read_attachments: &[(screenview, sampler), (screenview_d, sampler)],
            additional: vec![final_lit],
            camera_stage: ShaderStageFlags::VERTEX | ShaderStageFlags::FRAGMENT,
            frames_in_flight,
            ..Default::default()
        },
    )?);

    let post_processing_node = Box::new(CameraNode::<PostProcessingPass, _>::new(
        context.clone(),
        world,
        resources,
        camera,
        FullscreenRenderer::new(),
        CameraNodeInfo {
            name: "Post Processing node",
            color_attachments: color_attachments.to_vec(),
            read_attachments: &read_attachments.iter().map(|v| (*v, sampler)).collect_vec(),
            input_attachments: vec![final_lit, *depth_attachment],
            bindables: &data,
            frames_in_flight,
            camera_stage: ShaderStageFlags::FRAGMENT,
            ..Default::default()
        },
    )?);

    // Store data in camera
    world
        .insert(camera, (pbr_attachments, depth_attachment, env_manager))
        .expect("Entity is valid");

    Ok([
        camera_node,
        light_node,
        transfer,
        transfer_depth,
        transparent,
        post_processing_node,
    ])
}

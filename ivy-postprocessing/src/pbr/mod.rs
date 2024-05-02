use flax::{Component, Entity, World};
use itertools::Itertools;
use ivy_assets::{Asset, AssetCache};
use ivy_base::Extent;
use ivy_graphics::{
    DepthAttachment, EnvData, EnvironmentManager, FullscreenRenderer, GpuCamera, LightRenderer,
    MeshRenderer, Renderer,
};
use ivy_rendergraph::{AttachmentInfo, CameraNode, CameraNodeInfo, Node, TransferNode};
use ivy_vulkan::{
    context::SharedVulkanContext,
    descriptors::MultiDescriptorBindable,
    vk::{ClearValue, ImageAspectFlags, ShaderStageFlags},
    AddressMode, ClearValueExt, FilterMode, Format, ImageLayout, ImageUsage, LoadOp, Sampler,
    SamplerKey, Shader, StoreOp, Texture, TextureInfo,
};

mod attachments;

use crate::components::env_state;

use self::attachments::PBRAttachments;

#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct PBRInfo {
    pub max_lights: u64,
    pub output: Asset<Texture>,
    pub read_attachments: Vec<Asset<Texture>>,

    pub post_processing_shader: Shader,

    pub geometry_pass: Component<Shader>,
    pub transparent_pass: Component<Shader>,
}

/// Installs PBR rendering for the specified camera. Returns a list of nodes suitable for
/// rendergraph insertions. Configures gpu camera data, light management and
/// environment manager and attaches them to the camera.
pub fn setup_pbr_nodes<E, R>(
    context: SharedVulkanContext,
    world: &mut World,
    assets: &AssetCache,
    camera: Entity,
    renderer: R,
    extent: Extent,
    frames_in_flight: usize,
    info: PBRInfo,
    env: E,
) -> ivy_rendergraph::Result<impl IntoIterator<Item = Box<dyn Node>>>
where
    R: 'static + Send + Sync + Renderer,
    E: 'static + Send + Sync + Copy,
    E: EnvData,
{
    GpuCamera::create_gpu_cameras(&context, world, frames_in_flight)?;
    let pbr_attachments = PBRAttachments::new(context.clone(), assets, extent)?;

    let depth_attachment = DepthAttachment::new(context.clone(), assets, extent)?;

    let camera_node = Box::new(CameraNode::<R>::new(
        context.clone(),
        world,
        assets,
        camera,
        renderer,
        info.geometry_pass,
        CameraNodeInfo {
            name: "PBR Camera Node",
            color_attachments: pbr_attachments
                .as_slice()
                .iter()
                .zip([
                    ClearValue::color_vec4(env.clear_color().extend(1.0)),
                    ClearValue::default(),
                    ClearValue::default(),
                    ClearValue::default(),
                ])
                .map(|(resource, clear_value)| AttachmentInfo {
                    store_op: StoreOp::STORE,
                    load_op: LoadOp::CLEAR,
                    initial_layout: ImageLayout::UNDEFINED,
                    final_layout: ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                    resource: (*resource).clone(),
                    clear_value,
                })
                .collect::<Vec<_>>(),
            depth_attachment: Some(AttachmentInfo {
                store_op: StoreOp::STORE,
                load_op: LoadOp::CLEAR,
                initial_layout: ImageLayout::UNDEFINED,
                final_layout: ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
                resource: depth_attachment.0.clone(),
                clear_value: ClearValue::depth_stencil(1.0, 0),
            }),
            frames_in_flight,
            ..Default::default()
        },
    )?);

    let light_renderer = LightRenderer::new(context.clone(), info.max_lights, frames_in_flight)?;
    let env_manager = EnvironmentManager::new(context.clone(), env, frames_in_flight)?;

    let data = [&env_manager.buffers() as &dyn MultiDescriptorBindable];

    let input_attachments = [
        pbr_attachments.albedo,
        pbr_attachments.position,
        pbr_attachments.normal,
        pbr_attachments.roughness_metallic,
        depth_attachment.0.clone(),
    ];

    let sampler = assets.load(&SamplerKey {
        address_mode: AddressMode::CLAMP_TO_EDGE,
        mag_filter: FilterMode::LINEAR,
        min_filter: FilterMode::LINEAR,
        unnormalized_coordinates: false,
        anisotropy: 16,
        mip_levels: 1,
    });

    // TODO one image
    let final_lit = assets.insert(Texture::new(
        context.clone(),
        &TextureInfo {
            extent,
            mip_levels: 1,
            usage: ImageUsage::COLOR_ATTACHMENT
                | ImageUsage::INPUT_ATTACHMENT
                | ImageUsage::TRANSFER_SRC,
            ..Default::default()
        },
    )?);

    let light_node = Box::new(CameraNode::new(
        context.clone(),
        world,
        assets,
        camera,
        light_renderer,
        info.geometry_pass,
        CameraNodeInfo {
            name: "Light renderer",
            color_attachments: vec![AttachmentInfo::color(final_lit.clone())],
            input_attachments: input_attachments.to_vec(),
            camera_stage: ShaderStageFlags::VERTEX | ShaderStageFlags::FRAGMENT,
            frames_in_flight,
            ..Default::default()
        },
    )?);

    let screenview = assets.insert(Texture::new(
        context.clone(),
        &TextureInfo {
            extent,
            mip_levels: 1,
            usage: ImageUsage::TRANSFER_DST | ImageUsage::SAMPLED,
            ..Default::default()
        },
    )?);

    let screenview_d = assets.insert(Texture::new(
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
    )?);

    // Copy lit to the attachment to read
    let transfer = Box::new(TransferNode::new(
        final_lit.clone(),
        screenview.clone(),
        ImageLayout::TRANSFER_SRC_OPTIMAL,
        ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
        ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        ImageAspectFlags::COLOR,
    )?);

    // Copy lit to the attachment to read
    let transfer_depth = Box::new(TransferNode::new(
        depth_attachment.0.clone(),
        screenview_d.clone(),
        ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
        ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
        ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        ImageAspectFlags::DEPTH,
    )?);

    let mesh_renderer = MeshRenderer::new(context.clone(), 4, frames_in_flight)?;

    let transparent = Box::new(CameraNode::new(
        context.clone(),
        world,
        assets,
        camera,
        mesh_renderer,
        info.transparent_pass,
        CameraNodeInfo {
            name: "Transparent",
            color_attachments: vec![AttachmentInfo {
                store_op: StoreOp::STORE,
                load_op: LoadOp::LOAD,
                initial_layout: ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                final_layout: ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                resource: final_lit.clone(),
                clear_value: Default::default(),
            }],
            depth_attachment: Some(AttachmentInfo {
                resource: depth_attachment.0.clone(),
                store_op: StoreOp::STORE,
                load_op: LoadOp::LOAD,
                initial_layout: ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
                final_layout: ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
                clear_value: Default::default(),
            }),
            read_attachments: &[
                (screenview, sampler.clone()),
                (screenview_d, sampler.clone()),
            ],
            additional: vec![final_lit.clone()],
            camera_stage: ShaderStageFlags::VERTEX | ShaderStageFlags::FRAGMENT,
            frames_in_flight,
            ..Default::default()
        },
    )?);

    // TODO: remove once wgpu redesigns this slight mess that I made :P
    flax::component! {
        dummy_pass: Shader,
    }

    let post_processing_node = Box::new(CameraNode::new(
        context.clone(),
        world,
        assets,
        camera,
        FullscreenRenderer::new((*info.post_processing_shader.pipeline_info).clone()),
        dummy_pass(),
        CameraNodeInfo {
            name: "Post Processing node",
            color_attachments: vec![AttachmentInfo::color(info.output)],
            read_attachments: &info
                .read_attachments
                .iter()
                .map(|v| (v.clone(), sampler.clone()))
                .collect_vec(),
            input_attachments: vec![final_lit.clone(), depth_attachment.0.clone()],
            bindables: &data,
            frames_in_flight,
            camera_stage: ShaderStageFlags::FRAGMENT,
            ..Default::default()
        },
    )?);

    let mut camera = world.entity_mut(camera).unwrap();

    camera.set(
        ivy_graphics::components::depth_attachment(),
        depth_attachment,
    );
    camera.set(env_state(), env_manager);

    // Store data in camera
    // world
    //     .set(camera, (pbr_attachments, depth_attachment, env_manager))
    //     .expect("Entity is valid");

    Ok([
        camera_node,
        light_node,
        transfer,
        transfer_depth,
        transparent,
        post_processing_node,
    ] as [Box<dyn Node>; 6])
}

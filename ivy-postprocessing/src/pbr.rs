use std::sync::Arc;

use anyhow::Result;
use derive_more::{AsRef, Deref, From, Into};
use hecs::{Entity, World};
use ivy_graphics::{GpuCameraData, IndirectMeshRenderer, LightManager, ShaderPass};
use ivy_rendergraph::{AttachmentInfo, CameraNode, Node};
use ivy_resources::{Handle, Resources};
use ivy_vulkan::{
    descriptors::MultiDescriptorBindable, ClearValue, Extent, Format, ImageLayout, ImageUsage,
    LoadOp, SampleCountFlags, StoreOp, Texture, TextureInfo, VulkanContext,
};
use ultraviolet::Vec3;

use crate::node::PostProcessingNode;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deref, AsRef, Into, From)]
pub struct DepthAttachment(pub Handle<Texture>);

impl DepthAttachment {
    pub fn new(context: Arc<VulkanContext>, resources: &Resources, extent: Extent) -> Result<Self> {
        Ok(Self(resources.insert(Texture::new(
            context.clone(),
            &TextureInfo {
                extent,
                mip_levels: 1,
                usage: ImageUsage::DEPTH_STENCIL_ATTACHMENT | ImageUsage::SAMPLED,
                format: Format::D32_SFLOAT,
                samples: SampleCountFlags::TYPE_1,
            },
        )?)?))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PBRAttachments {
    pub albedo: Handle<Texture>,
    pub position: Handle<Texture>,
    pub normal: Handle<Texture>,
    pub roughness_metallic: Handle<Texture>,
}

impl PBRAttachments {
    pub fn new(context: Arc<VulkanContext>, resources: &Resources, extent: Extent) -> Result<Self> {
        let albedo = resources.insert(Texture::new(
            context.clone(),
            &TextureInfo {
                extent,
                mip_levels: 1,
                usage: ImageUsage::COLOR_ATTACHMENT | ImageUsage::INPUT_ATTACHMENT,
                format: Format::R8G8B8A8_SRGB,
                samples: SampleCountFlags::TYPE_1,
            },
        )?)?;

        let position = resources.insert(Texture::new(
            context.clone(),
            &TextureInfo {
                extent,
                mip_levels: 1,
                usage: ImageUsage::COLOR_ATTACHMENT | ImageUsage::INPUT_ATTACHMENT,
                format: Format::R32G32B32A32_SFLOAT,
                samples: SampleCountFlags::TYPE_1,
            },
        )?)?;

        let normal = resources.insert(Texture::new(
            context.clone(),
            &TextureInfo {
                extent,
                mip_levels: 1,
                usage: ImageUsage::COLOR_ATTACHMENT | ImageUsage::INPUT_ATTACHMENT,
                format: Format::R32G32B32A32_SFLOAT,
                samples: SampleCountFlags::TYPE_1,
            },
        )?)?;

        let roughness_metallic = resources.insert(Texture::new(
            context.clone(),
            &TextureInfo {
                extent,
                mip_levels: 1,
                usage: ImageUsage::COLOR_ATTACHMENT | ImageUsage::INPUT_ATTACHMENT,
                format: Format::R8G8_UNORM,
                samples: SampleCountFlags::TYPE_1,
            },
        )?)?;

        Ok(Self {
            albedo,
            position,
            normal,
            roughness_metallic,
        })
    }

    pub fn as_slice(&self) -> [Handle<Texture>; 4] {
        [
            self.albedo,
            self.position,
            self.normal,
            self.roughness_metallic,
        ]
    }
}

pub struct PBRInfo {
    pub ambient_radience: Vec3,
    pub max_lights: u64,
}

impl Default for PBRInfo {
    fn default() -> Self {
        Self {
            ambient_radience: Vec3::one() * 0.1,
            max_lights: 10,
        }
    }
}

/// Installs PBR rendering for the specified camera. Returns a list of nodes suitable for
/// rendergraph insertions. Configures gpu camera data and light management.
pub fn create_pbr_pipeline<GeometryPass: ShaderPass, PostProcessingPass: ShaderPass>(
    context: Arc<VulkanContext>,
    world: &mut World,
    resources: &Resources,
    camera: Entity,
    extent: Extent,
    frames_in_flight: usize,
    read_attachments: &[Handle<Texture>],
    color_attachments: &[AttachmentInfo],
    bindables: &[&dyn MultiDescriptorBindable],
    info: PBRInfo,
) -> Result<[Box<dyn Node>; 2]> {
    let pbr_attachments = PBRAttachments::new(context.clone(), resources, extent)?;

    let depth_attachment = DepthAttachment::new(context.clone(), resources, extent)?;

    let camera_node = Box::new(CameraNode::<GeometryPass, IndirectMeshRenderer>::new(
        camera,
        resources.default::<IndirectMeshRenderer>()?,
        pbr_attachments
            .as_slice()
            .iter()
            .map(|resource| AttachmentInfo {
                store_op: StoreOp::STORE,
                load_op: LoadOp::CLEAR,
                initial_layout: ImageLayout::UNDEFINED,
                final_layout: ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                resource: *resource,
            })
            .collect::<Vec<_>>(),
        vec![],
        vec![],
        Some(AttachmentInfo {
            store_op: StoreOp::STORE,
            load_op: LoadOp::CLEAR,
            initial_layout: ImageLayout::UNDEFINED,
            final_layout: ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            resource: *depth_attachment,
        }),
        vec![
            ClearValue::Color(0.0, 0.0, 0.0, 0.0).into(),
            ClearValue::Color(0.0, 0.0, 0.0, 0.0).into(),
            ClearValue::Color(0.0, 0.0, 0.0, 0.0).into(),
            ClearValue::Color(0.0, 0.0, 0.0, 0.0).into(),
            ClearValue::DepthStencil(1.0, 0).into(),
        ],
    ));

    let light_manager = LightManager::new(
        context.clone(),
        info.max_lights,
        info.ambient_radience,
        frames_in_flight,
    )?;

    let camera_data = GpuCameraData::new(context.clone(), frames_in_flight)?;
    let data = [
        camera_data.buffers(),
        light_manager.scene_buffers(),
        light_manager.light_buffers(),
    ];

    let bindables = data
        .iter()
        .map(|val| val as &dyn MultiDescriptorBindable)
        .chain(bindables.into_iter().cloned())
        .collect::<Vec<_>>();

    let post_processing_node = Box::new(PostProcessingNode::<PostProcessingPass>::new(
        context.clone(),
        resources,
        read_attachments,
        &pbr_attachments.as_slice(),
        &bindables,
        color_attachments,
        frames_in_flight,
    )?);

    // Store data in camera
    world.insert(camera, (light_manager, camera_data, pbr_attachments))?;

    Ok([camera_node, post_processing_node])
}

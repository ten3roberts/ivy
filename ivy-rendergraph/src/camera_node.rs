use crate::Result;
use std::{any::type_name, iter::once};

use anyhow::Context;
use flax::{Component, Entity, World};
use itertools::Itertools;
use ivy_assets::{Asset, AssetCache};
use ivy_graphics::{components::gpu_camera, Renderer};
use ivy_vulkan::{
    context::SharedVulkanContext,
    descriptors::{DescriptorBuilder, DescriptorSet, MultiDescriptorBindable},
    vk::{self, ClearValue, ShaderStageFlags},
    CombinedImageSampler, InputAttachment, PassInfo, Sampler, Shader, Texture,
};

use crate::{AttachmentInfo, Node, NodeKind};

pub struct CameraNodeInfo<'a> {
    pub name: &'static str,
    pub color_attachments: Vec<AttachmentInfo>,
    pub read_attachments: &'a [(Asset<Texture>, Asset<Sampler>)],
    pub input_attachments: Vec<Asset<Texture>>,
    pub depth_attachment: Option<AttachmentInfo>,
    pub buffer_reads: Vec<vk::Buffer>,
    pub bindables: &'a [&'a dyn MultiDescriptorBindable],
    pub clear_values: Vec<ClearValue>,
    pub frames_in_flight: usize,

    pub additional: Vec<Asset<Texture>>,
    pub camera_stage: ShaderStageFlags,
}

impl<'a> Default for CameraNodeInfo<'a> {
    fn default() -> Self {
        Self {
            name: "CameraNode",
            color_attachments: Default::default(),
            read_attachments: Default::default(),
            input_attachments: Default::default(),
            depth_attachment: Default::default(),
            buffer_reads: Default::default(),
            bindables: Default::default(),
            clear_values: Default::default(),
            frames_in_flight: Default::default(),
            additional: vec![],
            camera_stage: ShaderStageFlags::VERTEX,
        }
    }
}

/// Renders the scene with the given mesh pass using the provided camera.
pub struct CameraNode<R> {
    name: &'static str,
    renderer: R,
    pass: Component<Shader>,
    color_attachments: Vec<AttachmentInfo>,
    read_attachments: Vec<Asset<Texture>>,
    input_attachments: Vec<Asset<Texture>>,
    depth_attachment: Option<AttachmentInfo>,
    buffer_reads: Vec<vk::Buffer>,
    clear_values: Vec<ClearValue>,
    sets: Vec<DescriptorSet>,
}

impl<R> CameraNode<R>
where
    R: Renderer,
{
    pub fn new(
        context: SharedVulkanContext,
        world: &mut World,
        assets: &AssetCache,
        camera: Entity,
        renderer: R,
        shaderpass: Component<Shader>,
        info: CameraNodeInfo<'_>,
    ) -> Result<Self> {
        let combined_image_samplers = info
            .read_attachments
            .iter()
            .map(|val| -> Result<_> { Ok(CombinedImageSampler::new(&*val.0, &*val.1)) })
            .collect::<Result<Vec<_>>>()?;

        let input_bindabled = info
            .input_attachments
            .iter()
            .map(|val| -> Result<_> { Ok(InputAttachment::new(&**val)) })
            .collect::<Result<Vec<_>>>()?;

        let gpu_camera = world
            .get(camera, gpu_camera())
            .context("Missing GpuCamera component")?;

        let camera_buffers = gpu_camera.buffers();
        let bindables = once(&camera_buffers)
            .map(|v| (v as &dyn MultiDescriptorBindable, info.camera_stage))
            .chain(
                combined_image_samplers
                    .iter()
                    .map(|v| v as &dyn MultiDescriptorBindable)
                    .chain(
                        input_bindabled
                            .iter()
                            .map(|val| val as &dyn MultiDescriptorBindable),
                    )
                    .chain(info.bindables.iter().cloned())
                    .map(|val| (val, ShaderStageFlags::FRAGMENT)),
            )
            .collect::<Vec<_>>();

        let sets = DescriptorBuilder::from_mutliple_resources(
            &context,
            &bindables,
            info.frames_in_flight,
        )?;

        let clear_values = info
            .color_attachments
            .iter()
            .chain(info.depth_attachment.as_ref())
            .map(|v| v.clear_value)
            .collect();

        Ok(Self {
            name: info.name,
            sets,
            renderer,
            pass: shaderpass,
            color_attachments: info.color_attachments,
            read_attachments: info
                .read_attachments
                .iter()
                .map(|val| val.0.clone())
                .chain(info.additional)
                .collect_vec(),
            input_attachments: info.input_attachments,
            depth_attachment: info.depth_attachment,
            buffer_reads: info.buffer_reads,
            clear_values,
        })
    }
}

impl<R> Node for CameraNode<R>
where
    R: 'static + Send + Sync + Renderer,
{
    fn color_attachments(&self) -> &[AttachmentInfo] {
        &self.color_attachments
    }

    fn read_attachments(&self) -> &[Asset<Texture>] {
        &self.read_attachments
    }

    fn input_attachments(&self) -> &[Asset<Texture>] {
        &self.input_attachments
    }

    fn depth_attachment(&self) -> Option<&AttachmentInfo> {
        self.depth_attachment.as_ref()
    }

    fn buffer_reads(&self) -> &[vk::Buffer] {
        &self.buffer_reads
    }

    fn clear_values(&self) -> &[ivy_vulkan::vk::ClearValue] {
        &self.clear_values
    }

    fn node_kind(&self) -> crate::NodeKind {
        NodeKind::Graphics
    }

    fn debug_name(&self) -> &'static str {
        self.name
    }

    fn execute(
        &mut self,
        world: &mut World,
        assets: &AssetCache,
        cmd: &ivy_vulkan::commands::CommandBuffer,
        pass_info: &PassInfo,
        current_frame: usize,
    ) -> anyhow::Result<()> {
        self.renderer
            .draw(
                world,
                assets,
                cmd,
                &[self.sets[current_frame]],
                pass_info,
                &[],
                current_frame,
                self.pass,
            )
            .with_context(|| {
                format!(
                    "CameraNode failed to draw using supplied renderer: {:?}",
                    type_name::<R>()
                )
            })?;

        Ok(())
    }
}

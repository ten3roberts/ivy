use crate::Result;
use std::{any::type_name, iter::once, marker::PhantomData, ops::Deref};

use anyhow::Context;
use hecs::{Entity, World};
use itertools::Itertools;
use ivy_graphics::{GpuCamera, Renderer};
use ivy_resources::{Handle, Resources, Storage};
use ivy_vulkan::{
    context::SharedVulkanContext,
    descriptors::{DescriptorBuilder, DescriptorSet, MultiDescriptorBindable},
    shaderpass::ShaderPass,
    vk::{self, ClearValue, ShaderStageFlags},
    CombinedImageSampler, InputAttachment, PassInfo, Sampler, Texture,
};

use crate::{AttachmentInfo, Node, NodeKind};

pub struct CameraNodeInfo<'a> {
    pub name: &'static str,
    pub color_attachments: Vec<AttachmentInfo>,
    pub read_attachments: &'a [(Handle<Texture>, Handle<Sampler>)],
    pub input_attachments: Vec<Handle<Texture>>,
    pub depth_attachment: Option<AttachmentInfo>,
    pub buffer_reads: Vec<vk::Buffer>,
    pub bindables: &'a [&'a dyn MultiDescriptorBindable],
    pub clear_values: Vec<ClearValue>,
    pub frames_in_flight: usize,

    pub additional: Vec<Handle<Texture>>,
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

/// A rendergraph node rendering the scene using the provided camera.
pub struct CameraNode<Pass, R: Renderer> {
    name: &'static str,
    renderer: R,
    marker: PhantomData<Pass>,
    color_attachments: Vec<AttachmentInfo>,
    read_attachments: Vec<Handle<Texture>>,
    input_attachments: Vec<Handle<Texture>>,
    depth_attachment: Option<AttachmentInfo>,
    buffer_reads: Vec<vk::Buffer>,
    clear_values: Vec<ClearValue>,
    sets: Vec<DescriptorSet>,
}

impl<Pass, R> CameraNode<Pass, R>
where
    Pass: ShaderPass + Storage,
    R: Renderer + Storage,
    R::Error: Into<anyhow::Error>,
{
    pub fn new<'a>(
        context: SharedVulkanContext,
        world: &mut World,
        resources: &Resources,
        camera: Entity,
        renderer: R,
        info: CameraNodeInfo<'a>,
    ) -> Result<Self> {
        let combined_image_samplers = info
            .read_attachments
            .iter()
            .map(|val| -> Result<_> {
                Ok(CombinedImageSampler::new(
                    &*resources.get(val.0)?,
                    &*resources.get(val.1)?,
                ))
            })
            .collect::<Result<Vec<_>>>()?;

        let input_bindabled = info
            .input_attachments
            .iter()
            .map(|val| -> Result<_> { Ok(InputAttachment::new(resources.get(*val)?.deref())) })
            .collect::<Result<Vec<_>>>()?;

        let camera_buffers = world.get::<GpuCamera>(camera).unwrap();

        let camera_buffers = camera_buffers.buffers();
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
                    .chain(info.bindables.into_iter().cloned())
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
            marker: PhantomData,
            color_attachments: info.color_attachments,
            read_attachments: info
                .read_attachments
                .iter()
                .map(|val| val.0)
                .chain(info.additional)
                .collect_vec(),
            input_attachments: info.input_attachments,
            depth_attachment: info.depth_attachment,
            buffer_reads: info.buffer_reads,
            clear_values,
        })
    }
}

impl<Pass, R> Node for CameraNode<Pass, R>
where
    Pass: ShaderPass + Storage,
    R: Renderer + Storage,
    R::Error: Into<anyhow::Error> + Storage,
{
    fn color_attachments(&self) -> &[AttachmentInfo] {
        &self.color_attachments
    }

    fn read_attachments(&self) -> &[Handle<Texture>] {
        &self.read_attachments
    }

    fn input_attachments(&self) -> &[Handle<Texture>] {
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
        world: &mut hecs::World,
        resources: &ivy_resources::Resources,
        cmd: &ivy_vulkan::commands::CommandBuffer,
        pass_info: &PassInfo,
        current_frame: usize,
    ) -> anyhow::Result<()> {
        self.renderer
            .draw::<Pass>(
                world,
                resources,
                cmd,
                &[self.sets[current_frame]],
                pass_info,
                &[],
                current_frame,
            )
            .map_err(|e| e.into())
            .context(format!(
                "CameraNode failed to draw using supplied renderer: {:?}",
                type_name::<R>()
            ))?;

        Ok(())
    }
}

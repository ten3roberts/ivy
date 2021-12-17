use crate::Result;
use std::{any::type_name, marker::PhantomData, ops::Deref, sync::Arc};

use anyhow::Context;
use hecs::Entity;
use itertools::Itertools;
use ivy_graphics::{GpuCameraData, Renderer};
use ivy_resources::{Handle, Resources, Storage};
use ivy_vulkan::{
    descriptors::{DescriptorBuilder, DescriptorSet, IntoSet, MultiDescriptorBindable},
    shaderpass::ShaderPass,
    vk::{self, ClearValue, ShaderStageFlags},
    CombinedImageSampler, InputAttachment, Sampler, Texture, VulkanContext,
};

use crate::{AttachmentInfo, Node, NodeKind};

/// A rendergraph node rendering the scene using the provided camera.
pub struct CameraNode<Pass, R: Renderer> {
    name: &'static str,
    camera: Entity,
    renderer: R,
    marker: PhantomData<Pass>,
    color_attachments: Vec<AttachmentInfo>,
    read_attachments: Vec<Handle<Texture>>,
    input_attachments: Vec<Handle<Texture>>,
    depth_attachment: Option<AttachmentInfo>,
    buffer_reads: Vec<vk::Buffer>,
    clear_values: Vec<ClearValue>,
    sets: Option<Vec<DescriptorSet>>,
}

impl<Pass, R> CameraNode<Pass, R>
where
    Pass: ShaderPass + Storage,
    R: Renderer + Storage,
    R::Error: Into<anyhow::Error>,
{
    pub fn new(
        name: &'static str,
        context: Arc<VulkanContext>,
        resources: &Resources,
        camera: Entity,
        renderer: R,
        color_attachments: &[AttachmentInfo],
        read_attachments: &[(Handle<Texture>, Handle<Sampler>)],
        input_attachments: &[Handle<Texture>],
        depth_attachment: Option<AttachmentInfo>,
        buffer_reads: &[vk::Buffer],
        bindables: &[&dyn MultiDescriptorBindable],
        clear_values: &[ClearValue],
        frames_in_flight: usize,
    ) -> Result<Self> {
        let combined_image_samplers = read_attachments
            .iter()
            .map(|val| -> Result<_> {
                Ok(CombinedImageSampler::new(
                    resources.get(val.0)?.deref(),
                    resources.get(val.1)?.deref(),
                ))
            })
            .collect::<Result<Vec<_>>>()?;

        let input_bindabled = input_attachments
            .iter()
            .map(|val| -> Result<_> { Ok(InputAttachment::new(resources.get(*val)?.deref())) })
            .collect::<Result<Vec<_>>>()?;

        let bindables = combined_image_samplers
            .iter()
            .map(|val| val as &dyn MultiDescriptorBindable)
            .chain(
                input_bindabled
                    .iter()
                    .map(|val| val as &dyn MultiDescriptorBindable),
            )
            .chain(bindables.into_iter().cloned())
            .map(|val| (val, ShaderStageFlags::FRAGMENT))
            .collect::<Vec<_>>();

        let sets = if !bindables.is_empty() {
            Some(DescriptorBuilder::from_mutliple_resources(
                &context,
                &bindables,
                frames_in_flight,
            )?)
        } else {
            None
        };

        Ok(Self {
            name,
            camera,
            sets,
            renderer,
            marker: PhantomData,
            color_attachments: color_attachments.to_owned(),
            read_attachments: read_attachments.iter().map(|(a, _)| *a).collect_vec(),
            input_attachments: input_attachments.to_owned(),
            depth_attachment,
            buffer_reads: buffer_reads.to_owned(),
            clear_values: clear_values.to_owned(),
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
        current_frame: usize,
    ) -> anyhow::Result<()> {
        let camera_set = world
            .get::<GpuCameraData>(self.camera)
            .context("Camera does not contain `GpuCameraData`")?
            .set(current_frame);
        if let Some(sets) = &self.sets {
            self.renderer
                .draw::<Pass>(
                    world,
                    cmd,
                    current_frame,
                    &[camera_set, sets[current_frame]],
                    &[],
                    resources,
                )
                .map_err(|e| e.into())
                .context(format!(
                    "CameraNode failed to draw using supplied renderer: {:?}",
                    type_name::<R>()
                ))?;
        } else {
            self.renderer
                .draw::<Pass>(world, cmd, current_frame, &[camera_set], &[], resources)
                .map_err(|e| e.into())
                .context(format!(
                    "CameraNode failed to draw using supplied renderer: {:?}",
                    type_name::<R>()
                ))?;
        }

        Ok(())
    }
}

use std::marker::PhantomData;

use anyhow::Context;
use hecs::Entity;
use ivy_graphics::{GpuCameraData, Renderer, ShaderPass};
use ivy_resources::{Handle, Storage};
use ivy_vulkan::{vk::ClearValue, Texture};

use crate::{AttachmentInfo, Node, NodeKind};

pub struct CameraNode<Pass, T> {
    camera: Entity,
    renderer: Handle<T>,
    marker: PhantomData<Pass>,
    color_attachments: Vec<AttachmentInfo>,
    read_attachments: Vec<Handle<Texture>>,
    depth_attachment: Option<AttachmentInfo>,
    clear_values: Vec<ClearValue>,
}

impl<Pass, T> CameraNode<Pass, T>
where
    Pass: ShaderPass + Storage,
    T: Renderer + Storage,
{
    pub fn new(
        camera: Entity,
        renderer: Handle<T>,
        color_attachments: Vec<AttachmentInfo>,
        read_attachments: Vec<Handle<Texture>>,
        depth_attachment: Option<AttachmentInfo>,
        clear_values: Vec<ClearValue>,
    ) -> Self {
        Self {
            camera,
            renderer,
            marker: PhantomData,
            color_attachments,
            read_attachments,
            depth_attachment,
            clear_values,
        }
    }
}

impl<Pass, T> Node for CameraNode<Pass, T>
where
    Pass: ShaderPass + Storage,
    T: Renderer + Storage,
{
    fn color_attachments(&self) -> &[AttachmentInfo] {
        &self.color_attachments
    }

    fn read_attachments(&self) -> &[Handle<Texture>] {
        &self.read_attachments
    }

    fn depth_attachment(&self) -> Option<&AttachmentInfo> {
        self.depth_attachment.as_ref()
    }

    fn clear_values(&self) -> &[ivy_vulkan::vk::ClearValue] {
        &self.clear_values
    }

    fn node_kind(&self) -> crate::NodeKind {
        NodeKind::Graphics
    }

    fn execute(
        &mut self,
        world: &mut hecs::World,
        cmd: &ivy_vulkan::commands::CommandBuffer,
        current_frame: usize,
        resources: &ivy_resources::Resources,
    ) -> anyhow::Result<()> {
        let camera_set = world
            .get::<GpuCameraData>(self.camera)
            .context("Camera does not contain `GpuCameraData`")?
            .set(current_frame);

        resources
            .get_mut(self.renderer)?
            .draw::<Pass>(world, cmd, current_frame, &[camera_set], &[], resources)
            .context("CameraNode failed to draw using supplied renderer")?;

        Ok(())
    }
}

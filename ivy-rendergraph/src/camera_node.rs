use std::{any::type_name, marker::PhantomData};

use anyhow::Context;
use hecs::Entity;
use ivy_graphics::{GpuCameraData, Renderer, ShaderPass};
use ivy_resources::{Handle, Storage};
use ivy_vulkan::{descriptors::IntoSet, vk::ClearValue, Texture};

use crate::{AttachmentInfo, Node, NodeKind};

/// A rendergraph node rendering the scene using the provided camera.
pub struct CameraNode<Pass, T: Renderer<Error = E>, E> {
    camera: Entity,
    renderer: T,
    marker: PhantomData<(Pass, E)>,
    color_attachments: Vec<AttachmentInfo>,
    read_attachments: Vec<Handle<Texture>>,
    input_attachments: Vec<Handle<Texture>>,
    depth_attachment: Option<AttachmentInfo>,
    clear_values: Vec<ClearValue>,
}

impl<Pass, T, E> CameraNode<Pass, T, E>
where
    Pass: ShaderPass + Storage,
    T: Renderer<Error = E> + Storage,
    E: Into<anyhow::Error>,
{
    pub fn new(
        camera: Entity,
        renderer: T,
        color_attachments: Vec<AttachmentInfo>,
        read_attachments: Vec<Handle<Texture>>,
        input_attachments: Vec<Handle<Texture>>,
        depth_attachment: Option<AttachmentInfo>,
        clear_values: Vec<ClearValue>,
    ) -> Self {
        Self {
            camera,
            renderer,
            marker: PhantomData,
            color_attachments,
            read_attachments,
            input_attachments,
            depth_attachment,
            clear_values,
        }
    }
}

impl<Pass, T, E> Node for CameraNode<Pass, T, E>
where
    Pass: ShaderPass + Storage,
    T: Renderer<Error = E> + Storage,
    E: 'static + Into<anyhow::Error> + Send + Sync,
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

    fn clear_values(&self) -> &[ivy_vulkan::vk::ClearValue] {
        &self.clear_values
    }

    fn node_kind(&self) -> crate::NodeKind {
        NodeKind::Graphics
    }

    fn debug_name(&self) -> &'static str {
        "camera node"
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

        self.renderer
            .draw::<Pass>(world, cmd, current_frame, &[camera_set], &[], resources)
            .map_err(|e| e.into())
            .context(format!(
                "CameraNode failed to draw using supplied renderer: {:?}",
                type_name::<T>()
            ))?;

        Ok(())
    }
}

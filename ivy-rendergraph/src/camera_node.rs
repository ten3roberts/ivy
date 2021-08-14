use std::marker::PhantomData;

use anyhow::Context;
use hecs::Entity;
use ivy_graphics::{GpuCameraData, Renderer, ShaderPass};
use ivy_resources::{Handle, Storage};

use crate::Node;

pub struct CameraNode<Pass, T> {
    camera: Entity,
    renderer: Handle<T>,
    marker: PhantomData<Pass>,
}

impl<Pass, T> CameraNode<Pass, T>
where
    Pass: ShaderPass + Storage,
    T: Renderer + Storage,
{
    pub fn new(camera: Entity, renderer: Handle<T>) -> Self {
        Self {
            camera,
            renderer,
            marker: PhantomData,
        }
    }
}

impl<Pass, T> Node for CameraNode<Pass, T>
where
    Pass: ShaderPass + Storage,
    T: Renderer + Storage,
{
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

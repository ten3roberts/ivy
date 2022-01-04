use std::{marker::PhantomData, sync::Arc};

use glam::Vec3;
use ivy_vulkan::{Buffer, BufferAccess::Mapped, BufferUsage, VulkanContext};

use crate::Result;

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(C, align(16))]
/// The environment data for the built in pbr shader
pub struct DefaultEnvData {
    pub ambient_radiance: Vec3,
    pub fog_density: f32,
    pub fog_color: Vec3,
}

impl Default for DefaultEnvData {
    fn default() -> Self {
        Self {
            ambient_radiance: Vec3::ONE * 0.01,
            fog_color: Vec3::ZERO,
            fog_density: 0.1,
        }
    }
}

/// Manages a certain kind of environment data's GPU side buffers
pub struct EnvironmentManager<Kind = DefaultEnvData> {
    buffers: Vec<Buffer>,
    marker: PhantomData<Kind>,
}

impl<EnvData: Copy + Send + Sync> EnvironmentManager<EnvData> {
    pub fn new(
        context: Arc<VulkanContext>,
        env_data: EnvData,
        frames_in_flight: usize,
    ) -> Result<Self> {
        let buffers = (0..frames_in_flight)
            .map(|_| -> Result<_> {
                Buffer::new(
                    context.clone(),
                    BufferUsage::UNIFORM_BUFFER,
                    Mapped,
                    &[env_data],
                )
                .map_err(|e| e.into())
            })
            .collect::<Result<_>>()?;

        Ok(Self {
            buffers,
            marker: PhantomData,
        })
    }

    /// Changes the value of the environment for this frame.
    /// For proper use, apply changes for all frames in flight succesively
    pub fn update(&mut self, data: EnvData, current_frame: usize) -> Result<()> {
        self.buffers[current_frame]
            .fill(0, &[data])
            .map_err(|e| e.into())
    }

    /// Get a reference to the environment manager's buffers.
    pub fn buffers(&self) -> &[Buffer] {
        self.buffers.as_ref()
    }
}

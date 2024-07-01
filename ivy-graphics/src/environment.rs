
use glam::Vec3;
use ivy_vulkan::{context::SharedVulkanContext, Buffer, BufferAccess::Mapped, BufferUsage};

use crate::Result;

pub trait EnvData {
    fn clear_color(&self) -> Vec3;
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(C)]
/// The environment data for the built in pbr shader
pub struct DefaultEnvData {
    pub ambient_radiance: Vec3,
    pub fog_density: f32,
    /// The color of the fog
    pub fog_color: Vec3,
    /// The rate at which the fog fades. A higher value makes visiblity plateu
    /// longer and then steeply fall.
    pub fog_gradient: f32,
}

impl EnvData for DefaultEnvData {
    fn clear_color(&self) -> Vec3 {
        self.fog_color
    }
}

impl Default for DefaultEnvData {
    fn default() -> Self {
        Self {
            ambient_radiance: Vec3::ONE * 0.01,
            fog_color: Vec3::ZERO,
            fog_density: 0.01,
            fog_gradient: 2.0,
        }
    }
}

/// Manages a certain kind of environment data's GPU side buffers
pub struct EnvironmentManager {
    buffers: Vec<Buffer>,
}

impl EnvironmentManager {
    pub fn new<Data: Copy>(
        context: SharedVulkanContext,
        env_data: Data,
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

        Ok(Self { buffers })
    }

    /// Changes the value of the environment for this frame.
    /// For proper use, apply changes for all frames in flight succesively
    pub fn update<Data>(&mut self, data: Data, current_frame: usize) -> Result<()> {
        self.buffers[current_frame]
            .fill(0, &[data])
            .map_err(|e| e.into())
    }

    /// Get a reference to the environment manager's buffers.
    pub fn buffers(&self) -> &[Buffer] {
        self.buffers.as_ref()
    }
}

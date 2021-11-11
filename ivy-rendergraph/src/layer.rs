use std::sync::Arc;

use hecs::World;
use ivy_base::{Events, Layer};
use ivy_graphics::{GpuCameraData, LightManager};
use ivy_resources::Resources;
use ivy_vulkan::{device, Swapchain, VulkanContext};

use crate::RenderGraph;

/// A layer that abstracts the graphics rendering.  Executes the
/// default rendergraph handle and properly acquires a swapchain image
/// and presents, ensuring proper synchronization.
/// [`frames_in_flight`] refers to the amount of frames that are
/// queued on the graphics card. More frames means that the graphics
/// card is kept busy and frames are produced more
/// consequtively. However, latency is increased.
pub struct GraphicsLayer {
    context: Arc<VulkanContext>,
    frames_in_flight: usize,
}

impl GraphicsLayer {
    pub fn new(
        _: &mut World,
        resources: &Resources,
        _: &mut Events,
        frames_in_flight: usize,
    ) -> anyhow::Result<Self> {
        let context = resources.get_default::<Arc<VulkanContext>>()?.clone();

        Ok(Self {
            context,
            frames_in_flight,
        })
    }
}
impl Layer for GraphicsLayer {
    fn on_update(
        &mut self,
        world: &mut hecs::World,
        resources: &mut ivy_resources::Resources,
        _events: &mut ivy_base::Events,
        _frame_time: std::time::Duration,
    ) -> anyhow::Result<()> {
        let context = resources.get_default::<Arc<VulkanContext>>()?;
        // Ensure gpu side data for cameras
        GpuCameraData::create_gpu_cameras(&context, world, self.frames_in_flight)?;

        let mut rendergraph = resources.get_default_mut::<RenderGraph>()?;

        let current_frame = rendergraph.begin()?;

        resources
            .get_default_mut::<Swapchain>()?
            .acquire_next_image(rendergraph.wait_semaphore(current_frame))?;

        GpuCameraData::update_all_system(world, current_frame)?;
        LightManager::update_all_system(world, current_frame)?;

        rendergraph.execute(world, resources)?;
        rendergraph.end()?;

        // // Present results
        resources.get_default::<Swapchain>()?.present(
            context.present_queue(),
            &[rendergraph.signal_semaphore(current_frame)],
        )?;

        Ok(())
    }
}

impl Drop for GraphicsLayer {
    fn drop(&mut self) {
        device::wait_idle(self.context.device()).expect("Failed to wait on device");
    }
}

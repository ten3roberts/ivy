use anyhow::Context;
use hecs::World;
use hecs_schedule::Schedule;
use ivy_base::{Events, Layer};
use ivy_graphics::{systems, GpuCamera, GraphicsEvent};
use ivy_resources::Resources;
use ivy_vulkan::{context::SharedVulkanContext, device, traits::Backend, Swapchain};
use ivy_window::Window;

use crate::{RenderGraph, Result};

/// A layer that abstracts the graphics rendering.  Executes the
/// default rendergraph handle and properly acquires a swapchain image
/// and presents, ensuring proper synchronization.
/// `frames_in_flight` refers to the amount of frames that are
/// queued on the graphics card. More frames means that the graphics
/// card is kept busy and frames are produced more
/// consequtively. However, latency is increased.
pub struct GraphicsLayer {
    context: SharedVulkanContext,
    frames_in_flight: usize,
    schedule: Schedule,
}

impl GraphicsLayer {
    pub fn new(
        _: &mut World,
        resources: &Resources,
        _: &mut Events,
        frames_in_flight: usize,
    ) -> anyhow::Result<Self> {
        let context = resources.get_default::<SharedVulkanContext>()?.clone();

        let schedule = Schedule::builder()
            .add_system(systems::update_view_matrices)
            .add_system(systems::add_bounds)
            .add_system(GpuCamera::update_all_system)
            .build();

        Ok(Self {
            context,
            frames_in_flight,
            schedule,
        })
    }

    pub fn execute_rendergraph(
        &mut self,
        world: &mut World,
        resources: &mut Resources,
    ) -> Result<()> {
        let rendergraph = resources.get_default::<RenderGraph>()?;

        let mut current_frame = rendergraph.begin()?;
        resources
            .get_default_mut::<Swapchain>()?
            .acquire_next_image(rendergraph.wait_semaphore(current_frame))?;

        drop(rendergraph);
        self.schedule
            .execute_seq((&mut *world, &mut current_frame, &mut *resources))
            .unwrap();

        let mut rendergraph = resources.get_default_mut::<RenderGraph>()?;

        rendergraph.execute(world, resources)?;
        rendergraph.end()?;

        // Present results
        resources.get_default::<Swapchain>()?.present(
            self.context.present_queue(),
            &[rendergraph.signal_semaphore(current_frame)],
        )?;

        Ok(())
    }
}
impl Layer for GraphicsLayer {
    fn on_update(
        &mut self,
        world: &mut hecs::World,
        resources: &mut ivy_resources::Resources,
        events: &mut ivy_base::Events,
        _frame_time: std::time::Duration,
    ) -> anyhow::Result<()> {
        // Ensure gpu side data for cameras
        GpuCamera::create_gpu_cameras(&self.context, world, self.frames_in_flight)?;

        let window = resources.get_default::<Window>()?;
        let extent = window.extent();
        drop(window);

        if extent.width == 0 || extent.height == 0 {
            return Ok(());
        }

        match self.execute_rendergraph(world, resources) {
            Ok(()) => Ok(()),
            Err(crate::Error::Vulkan(ivy_vulkan::Error::Vulkan(
                ivy_vulkan::vk::Result::SUBOPTIMAL_KHR
                | ivy_vulkan::vk::Result::ERROR_OUT_OF_DATE_KHR,
            ))) => {
                let window = resources.get_default::<Window>()?;
                eprintln!("Recreating swapchain");
                resources
                    .get_default_mut::<Swapchain>()?
                    .recreate(window.framebuffer_size())
                    .context("Failed to recreate swapchain")?;

                events.send(GraphicsEvent::SwapchainRecreation);

                Ok(())
            }
            Err(e) => Err(e).context("Failed to execute rendergraph"),
        }
    }
}

impl Drop for GraphicsLayer {
    fn drop(&mut self) {
        device::wait_idle(self.context.device()).expect("Failed to wait on device");
    }
}

use flax::{Schedule, World};
use ivy_assets::AssetCache;
use ivy_core::engine;
use ivy_graphics::components::{self, swapchain};
use ivy_vulkan::{context::SharedVulkanContext, device};

use crate::{components::render_graph, Result};

pub struct GraphicsDesc {
    pub frames_in_flight: usize,
}

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
    pub fn execute_rendergraph(
        &mut self,
        world: &mut World,
        assets: &mut AssetCache,
    ) -> Result<()> {
        let render_graph = world
            .get_mut(engine(), render_graph())
            .expect("Missing render graph")
            .clone();

        let mut render_graph = render_graph.lock();

        let mut current_frame = render_graph.begin()?;
        world
            .get_mut(engine(), swapchain())
            .unwrap()
            .acquire_next_image(render_graph.wait_semaphore(current_frame))?;

        self.schedule
            .execute_seq_with(&mut *world, (&mut current_frame, &mut *assets))
            .unwrap();

        render_graph.execute(world, assets)?;
        render_graph.end()?;

        let swapchain = world.get_mut(engine(), components::swapchain()).unwrap();
        // Present results
        swapchain.present(
            self.context.present_queue(),
            &[render_graph.signal_semaphore(current_frame)],
        )?;

        Ok(())
    }

    pub fn frames_in_flight(&self) -> usize {
        self.frames_in_flight
    }
}

pub struct GraphicsLayerDesc {
    pub frames_in_flight: usize,
}

// impl LayerDesc for GraphicsLayerDesc {
//     type Layer = GraphicsLayer;

//     fn register(self, _: &mut World, assets: &AssetCache) -> anyhow::Result<Self::Layer> {
//         let context = assets.service::<VulkanContextService>().context();

//         let schedule = Schedule::builder()
//             .with_system(systems::update_view_matrices())
//             .with_system(systems::add_bounds_system())
//             .with_system(GpuCamera::update_system())
//             .build();

//         Ok(GraphicsLayer {
//             context,
//             schedule,
//             frames_in_flight: self.frames_in_flight,
//         })
//     }
// }

// impl Layer for GraphicsLayer {
//     fn on_update(
//         &mut self,
//         world: &mut World,
//         assets: &mut AssetCache,
//         events: &mut Events,
//         _frame_time: Duration,
//     ) -> anyhow::Result<()> {
//         // Ensure gpu side data for cameras
//         GpuCamera::create_gpu_cameras(&self.context, world, self.frames_in_flight)?;

//         let extent = world.get(engine(), window())?.extent();

//         // Window is minimized, no need to render
//         if extent.width == 0 || extent.height == 0 {
//             return Ok(());
//         }

//         match self.execute_rendergraph(world, assets) {
//             Ok(()) => Ok(()),
//             Err(crate::Error::Vulkan(ivy_vulkan::Error::Vulkan(
//                 ivy_vulkan::vk::Result::SUBOPTIMAL_KHR
//                 | ivy_vulkan::vk::Result::ERROR_OUT_OF_DATE_KHR,
//             ))) => {
//                 let window = world.get(engine(), window())?;
//                 eprintln!("Recreating swapchain");
//                 let swapchain = &mut *world.get_mut(engine(), swapchain())?;
//                 swapchain
//                     .recreate(window.framebuffer_size())
//                     .context("Failed to recreate swapchain")?;

//                 events.send(GraphicsEvent::SwapchainRecreation);

//                 Ok(())
//             }
//             Err(e) => Err(e).context("Failed to execute rendergraph"),
//         }
//     }
// }

impl Drop for GraphicsLayer {
    fn drop(&mut self) {
        device::wait_idle(self.context.device()).expect("Failed to wait on device");
    }
}

use std::{
    sync::{mpsc, Arc},
    time::Duration,
};

use flax::World;
use glfw::{Glfw, WindowEvent};
use ivy_assets::AssetCache;
use ivy_base::{engine, AppEvent, Events, Layer};
use ivy_input::InputEvent;
use ivy_vulkan::{context::VulkanContextService, Swapchain, SwapchainInfo, VulkanContext};
use ivy_window::{Window, WindowInfo};
use parking_lot::RwLock;

use crate::components;

/// Customize behaviour of the window layer
#[derive(Debug, Clone, PartialEq)]
pub struct WindowLayerInfo {
    pub window: WindowInfo,
    pub swapchain: SwapchainInfo,
}

impl Default for WindowLayerInfo {
    fn default() -> Self {
        Self {
            window: WindowInfo::default(),
            swapchain: SwapchainInfo::default(),
        }
    }
}

/// Window and swapchain abstractions layer.
/// Manages glfw window and swapchain, as well as forwarding events.
/// **Note:** Not responsible for recreating the swapchain as this is better
/// done during present in for example GraphicsLayer
pub struct WindowLayer {
    glfw: Arc<RwLock<Glfw>>,
    events: mpsc::Receiver<(f64, WindowEvent)>,
}

impl WindowLayer {
    pub fn new(
        world: &mut World,
        assets: &AssetCache,
        info: WindowLayerInfo,
    ) -> anyhow::Result<Self> {
        let glfw = Arc::new(RwLock::new(glfw::init(glfw::FAIL_ON_ERRORS)?));
        let (window, events) = Window::new(glfw.clone(), info.window)?;

        let context = Arc::new(VulkanContext::new(&window)?);

        let swapchain = Swapchain::new(context.clone(), &window, info.swapchain)?;

        assets.register_service(VulkanContextService::new(context));

        world.set(engine(), components::window(), window)?;
        world.set(engine(), components::swapchain(), swapchain)?;

        Ok(Self { glfw, events })
    }
}

impl Layer for WindowLayer {
    fn on_update(
        &mut self,
        _world: &mut World,
        _: &mut AssetCache,
        events: &mut Events,
        _frame_time: Duration,
    ) -> anyhow::Result<()> {
        self.glfw.write().poll_events();

        for (_, event) in glfw::flush_messages(&self.events) {
            if let WindowEvent::Close = event {
                events.send(AppEvent::Exit);
            }

            let input_event = InputEvent::from(event);
            events.send(input_event);
        }

        Ok(())
    }
}

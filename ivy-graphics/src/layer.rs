use std::{
    sync::{mpsc, Arc},
    time::Duration,
};

use glfw::{Glfw, WindowEvent};
use hecs::World;
use ivy_base::{AppEvent, Events, Layer};
use ivy_resources::Resources;
use ivy_vulkan::{Swapchain, SwapchainInfo, VulkanContext};
use parking_lot::RwLock;

use crate::{Window, WindowInfo};

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
        glfw: Arc<RwLock<Glfw>>,
        resources: &Resources,
        info: WindowLayerInfo,
    ) -> anyhow::Result<Self> {
        let (window, events) = Window::new(glfw.clone(), info.window)?;

        let context = Arc::new(VulkanContext::new(&window)?);

        let swapchain = Swapchain::new(context.clone(), &window, info.swapchain)?;

        resources.insert(context)?;
        resources.insert(swapchain)?;
        resources.insert(window)?;

        Ok(Self { glfw, events })
    }
}

impl Layer for WindowLayer {
    fn on_update(
        &mut self,
        _world: &mut World,
        _: &mut Resources,
        events: &mut Events,
        _frame_time: Duration,
    ) -> anyhow::Result<()> {
        self.glfw.write().poll_events();

        for (_, event) in glfw::flush_messages(&self.events) {
            if let WindowEvent::Close = event {
                events.send(AppEvent::Exit);
            }

            events.send(event);
        }

        Ok(())
    }
}

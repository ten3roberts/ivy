use std::{
    sync::{mpsc, Arc},
    time::Duration,
};

use glfw::{Glfw, WindowEvent};
use hecs::World;
use ivy_base::{AppEvent, Events, Layer};
use ivy_resources::Resources;
use ivy_vulkan::VulkanContext;
use parking_lot::RwLock;

use crate::{Window, WindowInfo};

/// Customize behaviour of the window layer
#[derive(Debug, Clone, PartialEq)]
pub struct WindowLayerInfo {
    pub window: WindowInfo,
}

impl Default for WindowLayerInfo {
    fn default() -> Self {
        Self {
            window: WindowInfo::default(),
        }
    }
}

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

        resources.insert(context)?;
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

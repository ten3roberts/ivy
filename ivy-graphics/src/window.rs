use crate::Result;
use ash::vk::{Handle, SurfaceKHR};
use glfw::{ClientApiHint, CursorMode, Glfw, WindowEvent, WindowHint};
use ivy_base::Extent;
use ivy_vulkan::traits::Backend;
use parking_lot::RwLock;
#[cfg(feature = "serialize")]
use serde::{Deserialize, Serialize};
use std::{
    borrow::Cow,
    sync::{mpsc::Receiver, Arc},
};
use glam::Vec2;

use crate::Error;

pub struct Window {
    glfw: Arc<RwLock<Glfw>>,
    inner: glfw::Window,
}

unsafe impl Send for Window {}
unsafe impl Sync for Window {}

pub fn init() -> Result<Arc<RwLock<Glfw>>> {
    Ok(Arc::new(RwLock::new(glfw::init(glfw::FAIL_ON_ERRORS)?)))
}

impl Window {
    pub fn new(
        glfw: Arc<RwLock<Glfw>>,
        info: WindowInfo,
    ) -> Result<(Window, Receiver<(f64, WindowEvent)>)> {
        let mut glfw_mut = glfw.write();
        glfw_mut.window_hint(WindowHint::ClientApi(ClientApiHint::NoApi));

        glfw_mut.window_hint(WindowHint::Resizable(info.resizable));

        let extent = info
            .extent
            .or_else(|| {
                glfw_mut.with_primary_monitor(|_, monitor| {
                    let mode = monitor?.get_video_mode()?;
                    Some(Extent::new(mode.width, mode.height))
                })
            })
            .ok_or(Error::WindowCreation)?;

        let (mut window, events) = match info.mode {
            WindowMode::Windowed => glfw_mut.create_window(
                extent.width,
                extent.height,
                info.title.as_ref(),
                glfw::WindowMode::Windowed,
            ),
            WindowMode::Borderless => glfw_mut.with_primary_monitor(|glfw, monitor| {
                let monitor = monitor?;
                glfw.window_hint(WindowHint::Decorated(false));

                let mode = monitor.get_video_mode()?;

                glfw.window_hint(glfw::WindowHint::RedBits(Some(mode.red_bits)));
                glfw.window_hint(glfw::WindowHint::GreenBits(Some(mode.green_bits)));
                glfw.window_hint(glfw::WindowHint::BlueBits(Some(mode.blue_bits)));
                glfw.window_hint(glfw::WindowHint::RefreshRate(Some(mode.refresh_rate)));

                glfw.create_window(
                    extent.width,
                    extent.height,
                    info.title.as_ref(),
                    glfw::WindowMode::Windowed,
                )
            }),
            WindowMode::Fullscreen => glfw_mut.with_primary_monitor(|glfw, monitor| {
                let monitor = monitor?;
                let mode = monitor.get_video_mode()?;

                glfw.window_hint(glfw::WindowHint::RedBits(Some(mode.red_bits)));
                glfw.window_hint(glfw::WindowHint::GreenBits(Some(mode.green_bits)));
                glfw.window_hint(glfw::WindowHint::BlueBits(Some(mode.blue_bits)));
                glfw.window_hint(glfw::WindowHint::RefreshRate(Some(mode.refresh_rate)));

                glfw.create_window(
                    extent.width,
                    extent.height,
                    info.title.as_ref(),
                    glfw::WindowMode::FullScreen(monitor),
                )
            }),
        }
        .ok_or(Error::WindowCreation)?;

        window.set_all_polling(true);

        drop(glfw_mut);

        Ok((
            Self {
                glfw,
                inner: window,
            },
            events,
        ))
    }

    pub fn extent(&self) -> Extent {
        self.inner.get_size().into()
    }

    /// Returns the window aspect ration
    pub fn aspect(&self) -> f32 {
        let size = self.extent();
        size.width as f32 / size.height as f32
    }

    /// Returns the cursor position in pixels from the top left of the screen.
    pub fn cursor_pos(&self) -> Vec2 {
        let (x, y) = self.inner.get_cursor_pos();
        Vec2::new(x as f32, y as f32)
    }

    /// Returns the cursor position in normalized device coordinates.
    pub fn normalized_cursor_pos(&self) -> Vec2 {
        let pos = self.cursor_pos();
        let extent = self.extent();
        Vec2::new(
            (2.0 * pos.x) / extent.width as f32 - 1.0,
            1.0 - (2.0 * pos.y) / extent.height as f32,
        )
    }

    pub fn set_cursor_mode(&mut self, mode: CursorMode) {
        self.inner.set_cursor_mode(mode)
    }
}

impl Backend for Window {
    fn create_surface(&self, instance: &ash::Instance) -> ivy_vulkan::Result<ash::vk::SurfaceKHR> {
        let mut surface: u64 = 0_u64;
        let result = self.inner.create_window_surface(
            instance.handle().as_raw() as _,
            std::ptr::null(),
            &mut surface,
        );

        if result != ivy_vulkan::vk::Result::SUCCESS.as_raw() as u32 {
            return Err(ivy_vulkan::vk::Result::from_raw(result as i32).into());
        }

        Ok(SurfaceKHR::from_raw(surface))
    }

    fn framebuffer_size(&self) -> Extent {
        self.inner.get_framebuffer_size().into()
    }

    fn extensions(&self) -> Vec<String> {
        self.glfw
            .read()
            .get_required_instance_extensions()
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub enum WindowMode {
    Windowed,
    Borderless,
    Fullscreen,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub struct WindowInfo {
    /// The windows size. Set to none to use the monitors size.
    pub extent: Option<Extent>,
    /// If window should be resizable
    pub resizable: bool,
    pub mode: WindowMode,
    pub title: Cow<'static, str>,
}

impl Default for WindowInfo {
    fn default() -> Self {
        WindowInfo {
            extent: Some(Extent::new(800, 600)),
            resizable: true,
            mode: WindowMode::Windowed,
            title: "Ivy".into(),
        }
    }
}

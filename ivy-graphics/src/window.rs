use crate::Result;
use std::sync::mpsc;

use glfw::{ClientApiHint, Glfw, Window, WindowHint};
use ivy_vulkan::Extent;

use crate::Error;

/// This modules builds upon glfw to provide easier window creation.

pub enum WindowMode {
    Windowed,
    Borderless,
    Fullscreen,
}

pub struct WindowInfo {
    /// The windows size. Set to none to use the monitors size.
    pub extent: Option<Extent>,
    /// If window should be resizable
    pub resizable: bool,
    pub mode: WindowMode,
}

impl Default for WindowInfo {
    fn default() -> Self {
        WindowInfo {
            extent: Some(Extent::new(800, 600)),
            resizable: true,
            mode: WindowMode::Windowed,
        }
    }
}

/// Creates a glfw window using the provided info.
pub fn create(
    glfw: &mut Glfw,
    title: &str,
    info: WindowInfo,
) -> Result<(Window, mpsc::Receiver<(f64, glfw::WindowEvent)>)> {
    glfw.window_hint(WindowHint::ClientApi(ClientApiHint::NoApi));

    glfw.window_hint(WindowHint::Resizable(info.resizable));

    let extent = info
        .extent
        .or_else(|| {
            glfw.with_primary_monitor(|_, monitor| {
                let mode = monitor?.get_video_mode()?;
                Some(Extent::new(mode.width, mode.height))
            })
        })
        .ok_or(Error::WindowCreation)?;

    let (mut window, events) = match info.mode {
        WindowMode::Windowed => glfw.create_window(
            extent.width,
            extent.height,
            title,
            glfw::WindowMode::Windowed,
        ),
        WindowMode::Borderless => {
            glfw.window_hint(WindowHint::Decorated(false));
            glfw.create_window(
                extent.width,
                extent.height,
                title,
                glfw::WindowMode::Windowed,
            )
        }
        WindowMode::Fullscreen => glfw.with_primary_monitor(|glfw, monitor| {
            let monitor = monitor?;
            let mode = monitor.get_video_mode()?;

            glfw.window_hint(glfw::WindowHint::RedBits(Some(mode.red_bits)));
            glfw.window_hint(glfw::WindowHint::GreenBits(Some(mode.green_bits)));
            glfw.window_hint(glfw::WindowHint::BlueBits(Some(mode.blue_bits)));

            glfw.create_window(
                extent.width,
                extent.height,
                title,
                glfw::WindowMode::FullScreen(monitor),
            )
        }),
    }
    .ok_or(Error::WindowCreation)?;

    window.set_all_polling(true);
    Ok((window, events))
}

pub trait WindowExt {
    fn extent(&self) -> Extent;
}

impl WindowExt for Window {
    fn extent(&self) -> Extent {
        self.get_size().into()
    }
}

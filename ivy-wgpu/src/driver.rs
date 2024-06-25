use std::{collections::HashMap, sync::Arc, time::Instant};

use flax::{components::name, Entity};
use glam::{vec2, Vec2};
use ivy_core::{driver::Driver, App};
use ivy_input::types::{
    CursorEntered, CursorLeft, CursorMoved, KeyboardInput, MouseInput, MouseMotion, ScrollInput,
};
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalPosition,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    window::{CursorGrabMode, Window, WindowId},
};

use crate::{
    components::main_window,
    events::{ApplicationReady, RedrawEvent, ResizedEvent},
};

pub struct WinitDriver {}

impl WinitDriver {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for WinitDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl Driver for WinitDriver {
    fn enter(&mut self, app: &mut ivy_core::App) -> anyhow::Result<()> {
        let event_loop = EventLoop::new()?;

        event_loop.run_app(&mut WinitEventHandler {
            app,
            current_time: Instant::now(),
            windows: Default::default(),
            modifiers: Default::default(),
            scale_factor: 0.0,
            last_cursor_pos: None,
        })?;

        Ok(())
    }
}

pub struct WinitEventHandler<'a> {
    current_time: Instant,
    app: &'a mut App,
    windows: HashMap<WindowId, Entity>,
    modifiers: winit::keyboard::ModifiersState,
    scale_factor: f64,
    last_cursor_pos: Option<Vec2>,
}

impl<'a> ApplicationHandler for WinitEventHandler<'a> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        tracing::info!("Received resume event");

        let window = Arc::new(
            event_loop
                .create_window(Window::default_attributes().with_title("Ivy"))
                .unwrap(),
        );

        let entity = Entity::builder()
            .set(name(), "MainWindow".into())
            .set(
                crate::components::window(),
                WindowHandle {
                    window: window.clone(),
                    cursor_lock: Default::default(),
                },
            )
            .set_default(main_window())
            .spawn(&mut self.app.world);

        self.scale_factor = window.scale_factor();

        self.app.init().unwrap();

        if let Err(err) = self.app.emit(ApplicationReady(window.clone())) {
            tracing::error!("Error emitting window created event: {:?}", err);
            event_loop.exit();
        }

        self.windows.insert(window.id(), entity);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, wid: WindowId, event: WindowEvent) {
        if let Err(err) = self.process_event(event_loop, event, self.windows[&wid]) {
            tracing::error!("Error processing event: {:?}", err);
            event_loop.exit();
        }
    }

    fn device_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _: winit::event::DeviceId,
        event: winit::event::DeviceEvent,
    ) {
        if let Err(err) = self.process_device_event(event_loop, event) {
            tracing::error!("Error processing device event: {:?}", err);
            event_loop.exit();
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let new_time = Instant::now();
        let delta = new_time.duration_since(self.current_time);
        self.current_time = new_time;

        if let Err(err) = self.app.tick(delta) {
            tracing::error!("{err:?}");
            event_loop.exit();
        }
    }
}

impl<'a> WinitEventHandler<'a> {
    fn process_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        event: WindowEvent,
        window_id: Entity,
    ) -> anyhow::Result<()> {
        match event {
            WindowEvent::ActivationTokenDone {
                serial: _,
                token: _,
            } => todo!(),
            WindowEvent::Resized(size) => {
                self.app.emit(ResizedEvent {
                    physical_size: size,
                })?;
            }
            WindowEvent::Moved(_) => {}
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Destroyed => todo!(),
            WindowEvent::DroppedFile(_) => todo!(),
            WindowEvent::HoveredFile(_) => todo!(),
            WindowEvent::HoveredFileCancelled => todo!(),
            WindowEvent::Focused(focus) => {
                tracing::info!(?focus, "focus");
            }
            WindowEvent::KeyboardInput { event, .. } => {
                self.app.emit(KeyboardInput {
                    modifiers: self.modifiers,
                    key: event.logical_key,
                    state: event.state,
                })?;
            }
            WindowEvent::ModifiersChanged(mods) => {
                self.modifiers = mods.state();
            }
            WindowEvent::Ime(_) => {}
            WindowEvent::CursorMoved {
                device_id: _,
                position,
            } => {
                let logical_pos = position.to_logical(1.0);
                self.app.emit(CursorMoved {
                    position: vec2(logical_pos.x, logical_pos.y),
                })?;

                let window = &mut *self
                    .app
                    .world
                    .get_mut(window_id, crate::components::window())
                    .unwrap();

                window.cursor_lock.cursor_moved(&window.window, position);
            }
            WindowEvent::CursorEntered { device_id: _ } => {
                self.app.emit(CursorEntered)?;
            }
            WindowEvent::CursorLeft { device_id: _ } => {
                self.last_cursor_pos = None;
                self.app.emit(CursorLeft)?;
            }
            WindowEvent::MouseWheel { delta, .. } => {
                self.app.emit(ScrollInput {
                    delta: match delta {
                        winit::event::MouseScrollDelta::LineDelta(x, y) => vec2(x, y) * 4.0,
                        winit::event::MouseScrollDelta::PixelDelta(v) => {
                            let v = v.to_logical(self.scale_factor);
                            vec2(v.x, v.y)
                        }
                    },
                })?;
            }
            WindowEvent::MouseInput { state, button, .. } => self.app.emit(MouseInput {
                modifiers: self.modifiers,
                button,
                state,
            })?,
            WindowEvent::PinchGesture {
                device_id: _,
                delta: _,
                phase: _,
            } => todo!(),
            WindowEvent::PanGesture {
                device_id: _,
                delta: _,
                phase: _,
            } => todo!(),
            WindowEvent::DoubleTapGesture { device_id: _ } => todo!(),
            WindowEvent::RotationGesture {
                device_id: _,
                delta: _,
                phase: _,
            } => todo!(),
            WindowEvent::TouchpadPressure {
                device_id: _,
                pressure: _,
                stage: _,
            } => todo!(),
            WindowEvent::AxisMotion {
                device_id: _,
                axis: _,
                value: _,
            } => {}
            WindowEvent::Touch(_) => todo!(),
            WindowEvent::ScaleFactorChanged {
                scale_factor,
                inner_size_writer: _,
            } => {
                self.scale_factor = scale_factor;
            }
            WindowEvent::ThemeChanged(_) => {}
            WindowEvent::Occluded(_) => {}
            WindowEvent::RedrawRequested => {
                self.app.emit(RedrawEvent)?;
                let window = self
                    .app
                    .world
                    .get_mut(window_id, crate::components::window())
                    .unwrap();

                window.window.request_redraw();
            }
        }

        Ok(())
    }

    fn process_device_event(
        &mut self,
        _: &ActiveEventLoop,
        event: winit::event::DeviceEvent,
    ) -> anyhow::Result<()> {
        match event {
            winit::event::DeviceEvent::Added => {}
            winit::event::DeviceEvent::Removed => {}
            winit::event::DeviceEvent::MouseMotion { delta } => {
                self.app.emit(MouseMotion {
                    delta: vec2(delta.0 as _, delta.1 as _),
                })?;
            }
            winit::event::DeviceEvent::MouseWheel { delta: _ } => {}
            winit::event::DeviceEvent::Motion { axis: _, value: _ } => {}
            winit::event::DeviceEvent::Button {
                button: _,
                state: _,
            } => {}
            winit::event::DeviceEvent::Key(_) => {}
        }

        Ok(())
    }
}

#[derive(Default)]
struct CursorLock {
    last_pos: PhysicalPosition<f64>,
    manual_lock: bool,
}

impl CursorLock {
    fn cursor_moved(&mut self, window: &Window, pos: PhysicalPosition<f64>) {
        if self.manual_lock {
            window.set_cursor_position(self.last_pos).unwrap();
        } else {
            self.last_pos = pos;
        }
    }

    pub fn set_cursor_lock(&mut self, window: &Window, lock: bool) {
        if lock {
            if window.set_cursor_grab(CursorGrabMode::Locked).is_err() {
                window.set_cursor_grab(CursorGrabMode::Confined).unwrap();
                self.manual_lock = true;
            }
        } else {
            self.manual_lock = false;
            window.set_cursor_grab(CursorGrabMode::None).unwrap();
        }

        window.set_cursor_visible(!lock);
    }
}

pub struct WindowHandle {
    window: Arc<Window>,
    cursor_lock: CursorLock,
}

impl WindowHandle {
    pub fn window(&self) -> &Window {
        &self.window
    }

    pub fn set_cursor_lock(&mut self, lock: bool) {
        self.cursor_lock.set_cursor_lock(&self.window, lock)
    }
}

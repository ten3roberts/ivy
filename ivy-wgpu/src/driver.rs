use std::{sync::Arc, time::Instant};

use glam::vec2;
use ivy_base::{driver::Driver, App};
use winit::{
    application::ApplicationHandler,
    event::{Modifiers, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop},
    window::{Window, WindowId},
};

use crate::events::{
    ApplicationReady, CursorMoved, KeyboardInput, MouseInput, RedrawEvent, ResizedEvent,
};

pub struct WinitDriver {}

impl WinitDriver {
    pub fn new() -> Self {
        Self {}
    }
}

impl Driver for WinitDriver {
    fn enter(&mut self, app: &mut ivy_base::App) -> anyhow::Result<()> {
        let event_loop = EventLoop::new()?;

        event_loop.run_app(&mut WinitEventHandler {
            app,
            current_time: Instant::now(),
            window: None,
            modifiers: Default::default(),
        })?;

        Ok(())
    }
}

pub struct WinitEventHandler<'a> {
    current_time: Instant,
    app: &'a mut App,
    window: Option<Arc<Window>>,
    modifiers: winit::keyboard::ModifiersState,
}

impl<'a> ApplicationHandler for WinitEventHandler<'a> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        tracing::info!("Received resume event");

        let window = Arc::new(
            event_loop
                .create_window(Window::default_attributes().with_title("Ivy"))
                .unwrap(),
        );

        self.app.init().unwrap();

        if let Err(err) = self.app.emit(ApplicationReady(window.clone())) {
            tracing::error!("Error emitting window created event: {:?}", err);
            event_loop.exit();
        }

        self.window = Some(window);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if let Err(err) = self.process_event(event_loop, event) {
            tracing::error!("Error processing event: {:?}", err);
            event_loop.exit();
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let new_time = Instant::now();
        let delta = new_time.duration_since(self.current_time);
        self.current_time = new_time;

        if let Err(err) = self.app.tick(delta) {
            tracing::error!("Error ticking app: {:?}", err);
            event_loop.exit();
        }
    }
}

impl<'a> WinitEventHandler<'a> {
    fn process_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        event: WindowEvent,
    ) -> anyhow::Result<()> {
        match event {
            WindowEvent::ActivationTokenDone { serial, token } => todo!(),
            WindowEvent::Resized(size) => {
                tracing::info!(?size, "resize");
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
            WindowEvent::KeyboardInput {
                device_id,
                event,
                is_synthetic,
            } => {
                let event = KeyboardInput {
                    modifiers: self.modifiers,
                    key: event.logical_key,
                    state: event.state,
                };

                self.app.emit(event)?;
            }
            WindowEvent::ModifiersChanged(mods) => {
                self.modifiers = mods.state();
            }
            WindowEvent::Ime(_) => {}
            WindowEvent::CursorMoved {
                device_id: _,
                position,
            } => {
                let position = position.to_logical(1.0);
                self.app.emit(CursorMoved {
                    position: vec2(position.x, position.y),
                })?;
            }
            WindowEvent::CursorEntered { device_id } => {}
            WindowEvent::CursorLeft { device_id } => {}
            WindowEvent::MouseWheel {
                device_id,
                delta,
                phase,
            } => todo!(),
            WindowEvent::MouseInput {
                device_id,
                state,
                button,
            } => self.app.emit(MouseInput {
                modifiers: self.modifiers,
                button,
                state,
            })?,
            WindowEvent::PinchGesture {
                device_id,
                delta,
                phase,
            } => todo!(),
            WindowEvent::PanGesture {
                device_id,
                delta,
                phase,
            } => todo!(),
            WindowEvent::DoubleTapGesture { device_id } => todo!(),
            WindowEvent::RotationGesture {
                device_id,
                delta,
                phase,
            } => todo!(),
            WindowEvent::TouchpadPressure {
                device_id,
                pressure,
                stage,
            } => todo!(),
            WindowEvent::AxisMotion {
                device_id,
                axis,
                value,
            } => {}
            WindowEvent::Touch(_) => todo!(),
            WindowEvent::ScaleFactorChanged {
                scale_factor,
                inner_size_writer,
            } => {}
            WindowEvent::ThemeChanged(_) => {}
            WindowEvent::Occluded(_) => {}
            WindowEvent::RedrawRequested => {
                self.app.emit(RedrawEvent)?;
                self.window.as_mut().unwrap().request_redraw();
            }
        }

        Ok(())
    }
}

use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

use atomic_refcell::AtomicRefCell;
use flax::{components::name, Entity};
use glam::{vec2, Vec2};
use ivy_core::{
    components::{engine, request_capture_mouse},
    driver::Driver,
    App,
};
use ivy_input::types::{CursorMoved, InputEvent, KeyboardInput, MouseInput, ScrollMotion};
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalPosition,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    window::{CursorGrabMode, Window, WindowAttributes, WindowId},
};

use crate::{
    components::{main_window, window, window_cursor_position, window_size},
    events::{ApplicationReady, RedrawEvent, ResizedEvent},
};

pub struct WinitDriver {
    window_attributes: WindowAttributes,
}

impl WinitDriver {
    pub fn new(window_attributes: WindowAttributes) -> Self {
        Self { window_attributes }
    }
}

impl Default for WinitDriver {
    fn default() -> Self {
        Self::new(Default::default())
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
            stats: AppStats::new(16),
            main_window: Default::default(),
            window_attributes: self.window_attributes.clone(),
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
    stats: AppStats,
    main_window: Option<Entity>,
    window_attributes: WindowAttributes,
}

impl ApplicationHandler for WinitEventHandler<'_> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        tracing::info!("Received resume event");

        let window = Arc::new(
            event_loop
                .create_window(self.window_attributes.clone())
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
            .set_default(window_size())
            .set_default(window_cursor_position())
            .spawn(&mut self.app.world);

        self.scale_factor = window.scale_factor();

        self.app.init().unwrap();

        if let Err(err) = self.app.emit_event(ApplicationReady(window.clone())) {
            tracing::error!("Error emitting window created event: {:?}", err);
            event_loop.exit();
        }

        self.windows.insert(window.id(), entity);
        self.main_window = Some(entity);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, wid: WindowId, event: WindowEvent) {
        if let Err(err) = self.process_event(event_loop, event, self.windows[&wid]) {
            tracing::error!("Error processing event\n{err:?}");
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
        self.stats.record_frame(delta);

        if let Err(err) = self.app.tick(delta) {
            tracing::error!("{err:?}");
            event_loop.exit();
        }

        if let Some(w) = self.main_window {
            let handle = self.app.world.get(w, window()).unwrap();
            let lock = self
                .app
                .world
                .get_copy(engine(), request_capture_mouse())
                .unwrap_or_default();

            handle.set_cursor_lock(lock);

            let report = self.stats.report();
            handle.window.set_title(&format!(
                "{} - {:>4.1?} {:>4.1?} {:>4.1?}",
                self.app.name(),
                report.min_frame_time,
                report.average_frame_time,
                report.max_frame_time,
            ))
        }
    }
}

impl WinitEventHandler<'_> {
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
                let logical_size = size.to_logical(self.scale_factor);

                let window = self.app.world().entity(window_id).unwrap();
                *window.get_mut(window_size()).unwrap() = logical_size;

                self.app.emit_event(ResizedEvent {
                    physical_size: size,
                })?;
            }
            WindowEvent::Moved(_) => {}
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Destroyed => todo!(),
            WindowEvent::DroppedFile(_) => todo!(),
            WindowEvent::HoveredFile(_) => todo!(),
            WindowEvent::HoveredFileCancelled => todo!(),
            WindowEvent::Focused(_focus) => {}
            WindowEvent::KeyboardInput { event, .. } => {
                self.app.emit_event(InputEvent::Keyboard(KeyboardInput {
                    modifiers: self.modifiers,
                    key: event.logical_key,
                    state: event.state,
                    text: event.text,
                }))?;
            }
            WindowEvent::ModifiersChanged(mods) => {
                self.modifiers = mods.state();
                self.app.emit_event(InputEvent::ModifiersChanged(mods))?;
            }
            WindowEvent::Ime(_) => {}
            WindowEvent::CursorMoved {
                device_id: _,
                position,
            } => {
                let logical_pos = position.to_logical(1.0);
                let window_entity = self.app.world().entity(window_id).unwrap();

                let size;
                {
                    *window_entity.get_mut(window_cursor_position()).unwrap() = logical_pos;
                    size = window_entity.get_copy(window_size()).unwrap();
                    let window = &mut *window_entity.get_mut(crate::components::window()).unwrap();
                    window
                        .cursor_lock
                        .borrow_mut()
                        .cursor_moved(&window.window, position);
                }

                self.app.emit_event(InputEvent::CursorMoved(CursorMoved {
                    absolute_position: logical_pos,
                    normalized_position: vec2(logical_pos.x, logical_pos.y)
                        / vec2(size.width, size.height),
                }))?;
            }
            WindowEvent::CursorEntered { device_id: _ } => {
                self.app.emit_event(InputEvent::CursorEntered)?;
            }
            WindowEvent::CursorLeft { device_id: _ } => {
                self.last_cursor_pos = None;
                self.app.emit_event(InputEvent::CursorLeft)?;
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let (delta, line_delta) = match delta {
                    winit::event::MouseScrollDelta::LineDelta(x, y) => {
                        (vec2(x, y) * 4.0, vec2(x, y))
                    }
                    winit::event::MouseScrollDelta::PixelDelta(v) => {
                        let v = v.to_logical(self.scale_factor);
                        (vec2(v.x, v.y), vec2(v.x, v.y))
                    }
                };

                self.app
                    .emit_event(InputEvent::Scroll(ScrollMotion { delta, line_delta }))?;
            }
            WindowEvent::MouseInput { state, button, .. } => {
                self.app.emit_event(InputEvent::MouseButton(MouseInput {
                    modifiers: self.modifiers,
                    button,
                    state,
                }))?
            }
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
                self.app.emit_event(RedrawEvent)?;
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
                self.app
                    .emit_event(InputEvent::CursorDelta(vec2(delta.0 as _, delta.1 as _)))?;
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
                if let Err(err) = window.set_cursor_grab(CursorGrabMode::Confined) {
                    tracing::warn!("Faile to lock {err:?}");
                }
                self.manual_lock = true;
            }
        } else {
            self.manual_lock = false;
            window.set_cursor_grab(CursorGrabMode::None).unwrap();
        }

        window.set_cursor_visible(!lock);
    }
}

#[derive(Clone)]
pub struct WindowHandle {
    window: Arc<Window>,
    cursor_lock: Arc<AtomicRefCell<CursorLock>>,
}

impl WindowHandle {
    pub fn window(&self) -> &Window {
        &self.window
    }

    pub fn set_cursor_lock(&self, lock: bool) {
        self.cursor_lock
            .borrow_mut()
            .set_cursor_lock(&self.window, lock)
    }
}

struct AppStats {
    frames: Vec<AppFrame>,
    max_frames: usize,
}

impl AppStats {
    pub fn new(max_frames: usize) -> Self {
        Self {
            frames: Vec::with_capacity(max_frames),
            max_frames,
        }
    }

    fn record_frame(&mut self, frame_time: Duration) {
        if self.frames.len() >= self.max_frames {
            self.frames.remove(0);
        }
        self.frames.push(AppFrame { frame_time });
    }

    fn report(&self) -> StatsReport {
        let average = self
            .frames
            .iter()
            .map(|f| f.frame_time)
            .sum::<Duration>()
            .div_f32(self.frames.len() as f32);

        let min = self
            .frames
            .iter()
            .map(|f| f.frame_time)
            .min()
            .unwrap_or_default();
        let max = self
            .frames
            .iter()
            .map(|f| f.frame_time)
            .max()
            .unwrap_or_default();

        StatsReport {
            average_frame_time: average,
            min_frame_time: min,
            max_frame_time: max,
        }
    }
}

pub struct StatsReport {
    pub average_frame_time: Duration,
    pub min_frame_time: Duration,
    pub max_frame_time: Duration,
}

struct AppFrame {
    frame_time: Duration,
}

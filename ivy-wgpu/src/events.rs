use std::sync::Arc;

use glam::Vec2;
use ivy_base::layer::events::Event;
use winit::{
    dpi::PhysicalSize,
    event::{ElementState, MouseButton},
    keyboard::{Key, ModifiersState},
    window::Window,
};

#[derive(Debug, Clone)]
pub struct ApplicationReady(pub(crate) Arc<Window>);

#[derive(Debug, Clone)]
pub struct RedrawEvent;

#[derive(Debug, Clone)]
pub struct ResizedEvent {
    pub physical_size: PhysicalSize<u32>,
}

#[derive(Debug, Clone)]
pub struct KeyboardInput {
    pub modifiers: ModifiersState,
    pub key: Key,
    pub state: ElementState,
}

#[derive(Debug, Clone)]
pub struct CursorMoved {
    pub position: Vec2,
}

#[derive(Debug, Clone)]
pub struct MouseInput {
    pub modifiers: ModifiersState,
    pub button: MouseButton,
    pub state: ElementState,
}

impl Event for CursorMoved {}
impl Event for KeyboardInput {}
impl Event for MouseInput {}

impl Event for ApplicationReady {}
impl Event for RedrawEvent {}
impl Event for ResizedEvent {}

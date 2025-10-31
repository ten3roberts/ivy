use std::sync::Arc;

use ivy_core::layer::events::Event;
use winit::{
    dpi::{LogicalSize, PhysicalSize},
    window::Window,
};

#[derive(Debug, Clone)]
pub struct ApplicationReady(pub Arc<Window>);

#[derive(Debug, Clone)]
pub struct RedrawEvent;

#[derive(Debug, Clone)]
pub struct WindowResizedEvent {
    pub physical_size: PhysicalSize<u32>,
    pub logical_size: LogicalSize<f32>,
}

#[derive(Debug, Clone)]
pub struct ScaleFactorChangedEvent {
    pub scale_factor: f64,
}

impl Event for ApplicationReady {}
impl Event for RedrawEvent {}
impl Event for WindowResizedEvent {}
impl Event for ScaleFactorChangedEvent {}

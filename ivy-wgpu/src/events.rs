use std::sync::Arc;

use ivy_base::layer::events::Event;
use winit::{dpi::PhysicalSize, window::Window};

#[derive(Debug, Clone)]
pub struct ApplicationReady(pub(crate) Arc<Window>);

#[derive(Debug, Clone)]
pub struct RedrawEvent;

#[derive(Debug, Clone)]
pub struct ResizedEvent {
    pub physical_size: PhysicalSize<u32>,
}

impl Event for ApplicationReady {}
impl Event for RedrawEvent {}
impl Event for ResizedEvent {}
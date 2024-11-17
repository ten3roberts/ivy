use std::time::Duration;

use crate::layer::events::Event;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AppEvent {}

#[derive(Debug, Clone)]
/// Irregular update event
pub struct TickEvent(pub Duration);

#[derive(Debug, Clone)]
pub struct PostInitEvent;

impl Event for TickEvent {}
impl Event for PostInitEvent {}

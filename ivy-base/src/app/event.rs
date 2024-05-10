use crate::layer::events::Event;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AppEvent {}

#[derive(Debug, Clone)]
/// Irregular update event
pub struct TickEvent;

#[derive(Debug, Clone)]
pub struct InitEvent;

impl Event for TickEvent {}
impl Event for InitEvent {}

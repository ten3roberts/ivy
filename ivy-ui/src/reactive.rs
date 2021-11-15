use std::marker::PhantomData;

use anyhow::Context;
use flume::Receiver;
use hecs::World;
use ivy_base::{Events, Layer};
use ivy_resources::Resources;

use crate::{
    events::{WidgetEvent, WidgetEventKind},
    systems,
};

/// A struct specifying how a widget should react based on hover, press, and
/// release. The struct holds the values which will be used for each state
pub struct Reactive<T> {
    pub normal: T,
    pub pressed: T,
}

impl<T> Reactive<T> {
    pub fn new(normal: T, pressed: T) -> Self {
        Self { normal, pressed }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ReactiveState {
    Normal,
    Pressed,
}

impl ReactiveState {
    pub fn try_from_event(event: &WidgetEvent) -> Option<Self> {
        match event.kind() {
            WidgetEventKind::Pressed(_) => Some(Self::Pressed),
            WidgetEventKind::Released(_) | WidgetEventKind::ReleasedBackground(_) => {
                Some(Self::Normal)
            }
            _ => None,
        }
    }
}

impl<T: Copy> Reactive<T> {
    pub fn update(&self, val: &mut T, state: ReactiveState) {
        match state {
            ReactiveState::Normal => *val = self.normal,
            ReactiveState::Pressed => *val = self.pressed,
        }
    }
}

/// Layer abstraction for updating reactive components.
pub struct ReactiveLayer<T> {
    rx: Receiver<WidgetEvent>,
    marker: PhantomData<T>,
}

impl<T> ReactiveLayer<T> {
    pub fn new(_world: &mut World, _resources: &mut Resources, events: &mut Events) -> Self {
        let (tx, rx) = flume::unbounded();
        events.subscribe(tx);
        Self {
            rx,
            marker: PhantomData,
        }
    }
}

impl<T: 'static + Copy + Send + Sync> Layer for ReactiveLayer<T> {
    fn on_update(
        &mut self,
        world: &mut hecs::World,
        _resources: &mut ivy_resources::Resources,
        _events: &mut ivy_base::Events,
        _frame_time: std::time::Duration,
    ) -> anyhow::Result<()> {
        systems::reactive_system::<T, _>(world, self.rx.try_iter()).context(format!(
            "Failed to execute reactive layer for {:?}",
            std::any::type_name::<T>()
        ))
    }
}

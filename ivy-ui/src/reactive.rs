use std::marker::PhantomData;

use anyhow::Context;
use flume::Receiver;
use hecs::World;
use ivy_base::{Events, Layer};
use ivy_resources::Resources;

use crate::{events::WidgetEvent, systems, WidgetEventKind};

#[cfg(feature = "serialize")]
use serde::{Deserialize, Serialize};

/// A struct specifying how a widget should react based on hover, press, and
/// release. The struct holds the values which will be used for each state
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
#[derive(Clone, Copy, Debug)]
pub struct Reactive<T> {
    pub unfocused: T,
    pub focused: T,
}

impl<T: Default> Default for Reactive<T> {
    fn default() -> Self {
        Self {
            unfocused: Default::default(),
            focused: Default::default(),
        }
    }
}

impl<T> Reactive<T> {
    pub fn new(normal: T, pressed: T) -> Self {
        Self {
            unfocused: normal,
            focused: pressed,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub enum ReactiveState {
    Unfocused,
    Focused,
}

impl ReactiveState {
    pub fn try_from_event(event: &WidgetEvent) -> Option<Self> {
        match event.kind() {
            WidgetEventKind::Focus(true) => Some(Self::Focused),
            WidgetEventKind::Focus(false) => Some(Self::Unfocused),
            _ => None,
        }
    }
}

impl<T: Copy> Reactive<T> {
    pub fn update(&self, val: &mut T, state: ReactiveState) {
        match state {
            ReactiveState::Unfocused => *val = self.unfocused,
            ReactiveState::Focused => *val = self.focused,
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
        events.subscribe_custom(tx);
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
        _: &mut ivy_resources::Resources,
        _: &mut ivy_base::Events,
        _: std::time::Duration,
    ) -> anyhow::Result<()> {
        systems::reactive_system::<T, _>(world, self.rx.try_iter()).context(format!(
            "Failed to execute reactive layer for {:?}",
            std::any::type_name::<T>()
        ))
    }
}

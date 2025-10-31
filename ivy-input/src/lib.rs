mod bindings;
pub mod components;
pub mod error;
pub mod layer;
pub mod types;
mod vector;

use std::collections::BTreeSet;

pub use bindings::*;
use flax::{component::ComponentValue, signal::BoxedSignal, CommandBuffer, Component, EntityRef};
use glam::{IVec2, IVec3, Vec2, Vec3};
use types::{InputEvent, InputKind};

pub struct InputState {
    activations: Vec<Box<dyn ActionHandler>>,
}

impl InputState {
    pub fn new() -> Self {
        Self {
            activations: Vec::new(),
        }
    }

    pub fn with_action<T: ComponentValue + InputStimulus + PartialEq>(
        mut self,
        target: Component<T>,
        action: Action<T>,
    ) -> Self {
        self.activations
            .push(Box::new(ComponentActionHandler::new(target, action)));
        self
    }

    /// Execute a function whenever value changes
    pub fn with_trigger_action<
        T: InputStimulus,
        F: 'static + Send + Sync + FnMut(&EntityRef<'_>, &mut CommandBuffer, T) -> anyhow::Result<()>,
    >(
        mut self,
        action: Action<T>,
        func: F,
    ) -> Self {
        self.activations
            .push(Box::new(TriggerActionHandler::new(func, action)));
        self
    }

    pub fn with_signal_action<T: InputStimulus>(
        mut self,
        action: Action<T>,
        signal: Component<BoxedSignal<T>>,
    ) -> Self {
        self.activations
            .push(Box::new(SignalActionHandler::new(action, signal)));
        self
    }

    pub fn add_action<T: ComponentValue + InputStimulus + PartialEq>(
        &mut self,
        target: Component<T>,
        action: Action<T>,
    ) -> &mut Self {
        self.activations
            .push(Box::new(ComponentActionHandler::new(target, action)));
        self
    }

    pub fn add_trigger_action<
        F: 'static
            + Send
            + Sync
            + FnMut(&EntityRef<'_>, &mut CommandBuffer, bool) -> anyhow::Result<()>,
    >(
        &mut self,
        action: Action<bool>,
        callback: F,
    ) -> &mut Self {
        self.activations
            .push(Box::new(TriggerActionHandler::new(callback, action)));
        self
    }

    pub fn apply(&mut self, event: &InputEvent) {
        for activation in self.activations.iter_mut() {
            activation.apply_input(event);
        }
    }

    pub fn update(&mut self, entity: &EntityRef, cmd: &mut CommandBuffer) -> anyhow::Result<()> {
        for activation in &mut self.activations {
            activation.update(entity, cmd)?;
        }

        Ok(())
    }
}

impl Default for InputState {
    fn default() -> Self {
        Self::new()
    }
}

pub(crate) trait ActionHandler: 'static + Send + Sync {
    fn update(&mut self, entity: &EntityRef, cmd: &mut CommandBuffer) -> anyhow::Result<()>;
    fn apply_input(&mut self, event: &InputEvent);
}

pub type TriggerAction =
    Box<dyn Send + Sync + FnMut(&EntityRef<'_>, &mut CommandBuffer, bool) -> anyhow::Result<()>>;

pub(crate) struct TriggerActionHandler<T, F> {
    value: T,
    callback: F,
    action: Action<T>,
    _marker: std::marker::PhantomData<T>,
}

impl<T, F> TriggerActionHandler<T, F> {
    pub(crate) fn new(callback: F, action: Action<T>) -> Self
    where
        T: InputStimulus,
    {
        Self {
            callback,
            action,
            value: T::ZERO,
            _marker: std::marker::PhantomData,
        }
    }
}

impl<T, F> ActionHandler for TriggerActionHandler<T, F>
where
    T: InputStimulus,
    F: 'static + Send + Sync + FnMut(&EntityRef<'_>, &mut CommandBuffer, T) -> anyhow::Result<()>,
{
    fn update(&mut self, entity: &EntityRef, cmd: &mut CommandBuffer) -> anyhow::Result<()> {
        let new_value = self.action.read_stimulus();
        if new_value != self.value {
            self.value = new_value;
            (self.callback)(entity, cmd, new_value)?;
        }

        Ok(())
    }

    fn apply_input(&mut self, event: &InputEvent) {
        self.action.apply(event);
    }
}

pub(crate) struct SignalActionHandler<T> {
    value: T,
    signal: Component<BoxedSignal<T>>,
    action: Action<T>,
}

impl<T: InputStimulus> SignalActionHandler<T> {
    pub(crate) fn new(action: Action<T>, signal: Component<BoxedSignal<T>>) -> Self {
        Self {
            action,
            signal,
            value: T::ZERO,
        }
    }
}

impl<T: InputStimulus> ActionHandler for SignalActionHandler<T> {
    fn update(&mut self, entity: &EntityRef, cmd: &mut CommandBuffer) -> anyhow::Result<()> {
        let new_value = self.action.read_stimulus();
        if new_value != self.value {
            self.value = new_value;
            (entity.get_mut(self.signal)?).execute(*entity, cmd, new_value)?;
        }

        Ok(())
    }

    fn apply_input(&mut self, event: &InputEvent) {
        self.action.apply(event);
    }
}

pub(crate) struct ComponentActionHandler<T> {
    target: Component<T>,
    action: Action<T>,
}

impl<T> ComponentActionHandler<T> {
    pub(crate) fn new(target: Component<T>, action: Action<T>) -> Self {
        Self { target, action }
    }
}

impl<T: ComponentValue + InputStimulus + PartialEq> ActionHandler for ComponentActionHandler<T> {
    fn update(&mut self, entity: &EntityRef, cmd: &mut CommandBuffer) -> anyhow::Result<()> {
        let stimulus = self.action.read_stimulus();
        if entity.has(self.target) {
            entity.update_dedup(self.target, stimulus);
        } else {
            cmd.set(entity.id(), self.target, stimulus);
        }
        Ok(())
    }

    fn apply_input(&mut self, event: &InputEvent) {
        self.action.apply(event);
    }
}

impl<T: ComponentValue + InputStimulus + PartialEq> From<(Component<T>, Action<T>)>
    for ComponentActionHandler<T>
{
    fn from(v: (Component<T>, Action<T>)) -> Self {
        Self::new(v.0, v.1)
    }
}

pub struct Action<T> {
    bindings: Vec<Box<dyn Binding<Value = T>>>,
    binding_map: BTreeSet<(InputKind, usize)>,
}

impl<T> std::fmt::Debug for Action<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Action")
            .field("binding_map", &self.binding_map)
            .finish()
    }
}

impl<T: ComponentValue + InputStimulus> Action<T> {
    pub fn new() -> Self {
        Self {
            bindings: Vec::new(),
            binding_map: BTreeSet::new(),
        }
    }

    pub fn add(&mut self, action: impl 'static + Binding<Value = T>) -> &mut Self {
        let index = self.bindings.len();
        for binding in action.bindings() {
            self.binding_map.insert((binding, index));
        }

        self.bindings
            .push(Box::new(action) as Box<dyn Binding<Value = T>>);
        self
    }

    pub fn with_binding(mut self, action: impl 'static + Binding<Value = T>) -> Self {
        self.add(action);
        self
    }

    fn apply(&mut self, event: &InputEvent) {
        let kind = event.to_kind();
        for (_, binding) in self
            .binding_map
            .range((kind.clone(), usize::MIN)..(kind, usize::MAX))
        {
            self.bindings[*binding].apply(event);
        }
    }

    fn read_stimulus(&mut self) -> T {
        self.bindings
            .iter_mut()
            .fold(T::ZERO, |acc, binding| acc.combine(&binding.read()))
    }
}

impl<T: ComponentValue + InputStimulus> Default for Action<T> {
    fn default() -> Self {
        Self::new()
    }
}

pub trait InputStimulus: 'static + Send + Sync + ComponentValue + PartialEq + Copy {
    const ZERO: Self;
    fn combine(&self, other: &Self) -> Self;
}

impl InputStimulus for f32 {
    const ZERO: Self = 0.0;

    fn combine(&self, other: &Self) -> Self {
        self + other
    }
}

impl InputStimulus for bool {
    const ZERO: Self = false;

    fn combine(&self, other: &Self) -> Self {
        *self || *other
    }
}

impl InputStimulus for i32 {
    const ZERO: Self = 0;

    fn combine(&self, other: &Self) -> Self {
        self + other
    }
}

impl InputStimulus for Vec2 {
    const ZERO: Self = Vec2::ZERO;

    fn combine(&self, other: &Self) -> Self {
        *self + *other
    }
}

impl InputStimulus for Vec3 {
    const ZERO: Self = Vec3::ZERO;

    fn combine(&self, other: &Self) -> Self {
        *self + *other
    }
}

impl InputStimulus for IVec2 {
    const ZERO: Self = IVec2::ZERO;

    fn combine(&self, other: &Self) -> Self {
        *self + *other
    }
}

impl InputStimulus for IVec3 {
    const ZERO: Self = IVec3::ZERO;

    fn combine(&self, other: &Self) -> Self {
        *self + *other
    }
}

#[cfg(test)]
mod test {
    use winit::{
        event::ElementState,
        keyboard::{Key, KeyCode, PhysicalKey},
    };

    use crate::{types::KeyboardInput, Action, InputEvent, KeyBinding};

    #[test]
    fn input_state() {
        let mut activation = Action::new()
            .with_binding(KeyBinding::new(KeyCode::KeyA))
            .with_binding(KeyBinding::new(KeyCode::KeyB));

        activation.apply(&InputEvent::Keyboard(KeyboardInput {
            key: Key::Character("A".into()),
            physical_key: PhysicalKey::Code(KeyCode::KeyA),
            state: ElementState::Pressed,
            modifiers: Default::default(),
            text: Default::default(),
        }));

        assert!(activation.read_stimulus());

        activation.apply(&InputEvent::Keyboard(KeyboardInput {
            key: Key::Character("B".into()),
            physical_key: PhysicalKey::Code(KeyCode::KeyB),
            state: ElementState::Pressed,
            modifiers: Default::default(),
            text: Default::default(),
        }));

        assert!(activation.read_stimulus());

        activation.apply(&InputEvent::Keyboard(KeyboardInput {
            key: Key::Character("A".into()),
            physical_key: PhysicalKey::Code(KeyCode::KeyA),
            state: ElementState::Released,
            modifiers: Default::default(),
            text: Default::default(),
        }));

        assert!(activation.read_stimulus());
        activation.apply(&InputEvent::Keyboard(KeyboardInput {
            key: Key::Character("B".into()),
            physical_key: PhysicalKey::Code(KeyCode::KeyB),
            state: ElementState::Released,
            modifiers: Default::default(),
            text: Default::default(),
        }));

        assert!(!activation.read_stimulus());
    }
}

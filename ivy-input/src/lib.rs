mod bindings;
pub mod components;
pub mod error;
pub mod layer;
pub mod types;
mod vector;

use std::collections::BTreeSet;

pub use bindings::*;
use flax::{component::ComponentValue, Component, EntityRef};
use glam::{IVec2, IVec3, Vec2, Vec3};
use types::{InputEvent, InputKind};

pub struct InputState {
    activations: Vec<ActionKind>,
}

impl InputState {
    pub fn new() -> Self {
        Self {
            activations: Vec::new(),
        }
    }

    pub fn with_action<T>(mut self, target: Component<T>, action: Action<T>) -> Self
    where
        (Component<T>, Action<T>): Into<ActionKind>,
    {
        self.activations.push((target, action).into());
        self
    }

    pub fn apply(&mut self, event: &InputEvent) {
        for activation in self.activations.iter_mut() {
            match activation {
                ActionKind::Boolean(_, mapping) => mapping.apply(event),
                ActionKind::Integral(_, mapping) => mapping.apply(event),
                ActionKind::Scalar(_, mapping) => mapping.apply(event),
                ActionKind::Vector2(_, mapping) => mapping.apply(event),
                ActionKind::Vector3(_, mapping) => mapping.apply(event),
                ActionKind::IVector2(_, mapping) => mapping.apply(event),
                ActionKind::IVector3(_, mapping) => mapping.apply(event),
            }
        }
    }

    pub fn update(&mut self, entity: &EntityRef) -> anyhow::Result<()> {
        for activation in &mut self.activations {
            match activation {
                ActionKind::Boolean(target, m) => {
                    m.update(*target, entity)?;
                }
                ActionKind::Integral(target, m) => {
                    m.update(*target, entity)?;
                }
                ActionKind::Scalar(target, m) => {
                    m.update(*target, entity)?;
                }
                ActionKind::Vector2(target, m) => {
                    m.update(*target, entity)?;
                }
                ActionKind::Vector3(target, m) => {
                    m.update(*target, entity)?;
                }
                ActionKind::IVector2(target, m) => {
                    m.update(*target, entity)?;
                }
                ActionKind::IVector3(target, m) => {
                    m.update(*target, entity)?;
                }
            }
        }

        Ok(())
    }
}

impl Default for InputState {
    fn default() -> Self {
        Self::new()
    }
}

pub enum ActionKind {
    Boolean(Component<bool>, Action<bool>),
    Integral(Component<i32>, Action<i32>),
    Scalar(Component<f32>, Action<f32>),
    Vector2(Component<Vec2>, Action<Vec2>),
    Vector3(Component<Vec3>, Action<Vec3>),
    IVector2(Component<IVec2>, Action<IVec2>),
    IVector3(Component<IVec3>, Action<IVec3>),
}

impl From<(Component<bool>, Action<bool>)> for ActionKind {
    fn from(v: (Component<bool>, Action<bool>)) -> Self {
        Self::Boolean(v.0, v.1)
    }
}

impl From<(Component<i32>, Action<i32>)> for ActionKind {
    fn from(v: (Component<i32>, Action<i32>)) -> Self {
        Self::Integral(v.0, v.1)
    }
}

impl From<(Component<f32>, Action<f32>)> for ActionKind {
    fn from(v: (Component<f32>, Action<f32>)) -> Self {
        Self::Scalar(v.0, v.1)
    }
}

impl From<(Component<Vec2>, Action<Vec2>)> for ActionKind {
    fn from(v: (Component<Vec2>, Action<Vec2>)) -> Self {
        Self::Vector2(v.0, v.1)
    }
}

impl From<(Component<Vec3>, Action<Vec3>)> for ActionKind {
    fn from(v: (Component<Vec3>, Action<Vec3>)) -> Self {
        Self::Vector3(v.0, v.1)
    }
}

impl From<(Component<IVec2>, Action<IVec2>)> for ActionKind {
    fn from(v: (Component<IVec2>, Action<IVec2>)) -> Self {
        Self::IVector2(v.0, v.1)
    }
}

impl From<(Component<IVec3>, Action<IVec3>)> for ActionKind {
    fn from(v: (Component<IVec3>, Action<IVec3>)) -> Self {
        Self::IVector3(v.0, v.1)
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

impl<T: ComponentValue + Stimulus> Action<T> {
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
        // if let Some(&binding) = self.binding_map.range(&event.to_kind()) {}
    }

    fn get_stimulus(&mut self) -> T {
        self.bindings
            .iter_mut()
            .fold(T::ZERO, |acc, binding| acc.combine(&binding.read()))
    }

    fn update(
        &mut self,
        target: Component<T>,
        entity: &EntityRef,
    ) -> Result<(), error::MissingTargetError>
    where
        T: PartialEq,
    {
        entity
            .update_dedup(target, self.get_stimulus())
            .ok_or_else(|| error::MissingTargetError {
                target: target.desc(),
                entity: entity.id(),
            })
    }
}

impl<T: ComponentValue + Stimulus> Default for Action<T> {
    fn default() -> Self {
        Self::new()
    }
}

pub trait Stimulus {
    const ZERO: Self;
    fn combine(&self, other: &Self) -> Self;
}

impl Stimulus for f32 {
    const ZERO: Self = 0.0;

    fn combine(&self, other: &Self) -> Self {
        self + other
    }
}

impl Stimulus for bool {
    const ZERO: Self = false;

    fn combine(&self, other: &Self) -> Self {
        *self || *other
    }
}

impl Stimulus for i32 {
    const ZERO: Self = 0;

    fn combine(&self, other: &Self) -> Self {
        self + other
    }
}

impl Stimulus for Vec2 {
    const ZERO: Self = Vec2::ZERO;

    fn combine(&self, other: &Self) -> Self {
        *self + *other
    }
}

impl Stimulus for Vec3 {
    const ZERO: Self = Vec3::ZERO;

    fn combine(&self, other: &Self) -> Self {
        *self + *other
    }
}

impl Stimulus for IVec2 {
    const ZERO: Self = IVec2::ZERO;

    fn combine(&self, other: &Self) -> Self {
        *self + *other
    }
}

impl Stimulus for IVec3 {
    const ZERO: Self = IVec3::ZERO;

    fn combine(&self, other: &Self) -> Self {
        *self + *other
    }
}

#[cfg(test)]
mod test {
    use winit::{event::ElementState, keyboard::Key};

    use crate::{types::KeyboardInput, Action, InputEvent, KeyBinding};

    #[test]
    fn input_state() {
        let mut activation = Action::new()
            .with_binding(KeyBinding::new(Key::Character("A".into())))
            .with_binding(KeyBinding::new(Key::Character("B".into())));

        activation.apply(&InputEvent::Keyboard(KeyboardInput {
            key: Key::Character("A".into()),
            state: ElementState::Pressed,
            modifiers: Default::default(),
            text: Default::default(),
        }));

        assert!(activation.get_stimulus());

        activation.apply(&InputEvent::Keyboard(KeyboardInput {
            key: Key::Character("B".into()),
            state: ElementState::Pressed,
            modifiers: Default::default(),
            text: Default::default(),
        }));

        assert!(activation.get_stimulus());

        activation.apply(&InputEvent::Keyboard(KeyboardInput {
            key: Key::Character("A".into()),
            state: ElementState::Released,
            modifiers: Default::default(),
            text: Default::default(),
        }));

        assert!(activation.get_stimulus());
        activation.apply(&InputEvent::Keyboard(KeyboardInput {
            key: Key::Character("B".into()),
            state: ElementState::Released,
            modifiers: Default::default(),
            text: Default::default(),
        }));

        assert!(!activation.get_stimulus());
    }
}

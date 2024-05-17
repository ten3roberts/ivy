pub mod components;
mod events;
mod input;
mod types;
mod vector;

use std::collections::HashMap;

use flax::Component;
use glam::{Vec2, Vec3};

pub use events::*;
pub use input::*;
pub use vector::*;
use winit::{
    event::{ElementState, KeyEvent, MouseButton},
    keyboard::{Key, SmolStr},
};

pub struct InputState {
    activations: Vec<Activation>,
}

impl InputState {
    pub fn new() -> Self {
        Self {
            activations: Vec::new(),
        }
    }

    pub fn add(&mut self, activation: Activation) -> &mut Self {
        self.activations.push(activation);
        self
    }
}

pub enum Activation {
    Scalar(ActivationMapping<f32>),
    Vector2(ActivationMapping<Vec2>),
    Vector3(ActivationMapping<Vec3>),
}

impl From<ActivationMapping<f32>> for Activation {
    fn from(mapping: ActivationMapping<f32>) -> Self {
        Self::Scalar(mapping)
    }
}

impl From<ActivationMapping<Vec2>> for Activation {
    fn from(mapping: ActivationMapping<Vec2>) -> Self {
        Self::Vector2(mapping)
    }
}

impl From<ActivationMapping<Vec3>> for Activation {
    fn from(mapping: ActivationMapping<Vec3>) -> Self {
        Self::Vector3(mapping)
    }
}

pub struct ActivationMapping<T> {
    bindings: HashMap<InputKind, (Box<dyn Binding<T, Input = InputEvent>>, T)>,
}

impl<T: Stimulus> ActivationMapping<T> {
    pub fn new() -> Self {
        Self {
            bindings: HashMap::new(),
        }
    }

    pub fn add(&mut self, action: impl 'static + Binding<T, Input = InputEvent>) -> &mut Self {
        self.bindings.insert(
            action.binding(),
            (
                Box::new(action) as Box<dyn Binding<T, Input = InputEvent>>,
                T::ZERO,
            ),
        );
        self
    }

    fn apply(&mut self, event: InputEvent) {
        if let Some(binding) = self.bindings.get_mut(&event.to_kind()) {
            binding.1 = binding.0.apply(&event);
        }
    }

    fn get_stimulus(&self) -> T {
        self.bindings
            .values()
            .fold(T::ZERO, |acc, (_, value)| acc.combine(value))
    }
}

pub trait Binding<T> {
    type Input;

    fn apply(&self, input: &Self::Input) -> T;
    fn binding(&self) -> InputKind;
}

pub enum InputEvent {
    Key {
        key: Key,
        state: ElementState,
    },
    MouseButton {
        button: MouseButton,
        state: ElementState,
    },
    CursorPos(Vec2),
}

impl InputEvent {
    fn to_kind(&self) -> InputKind {
        match self {
            InputEvent::Key { key, .. } => InputKind::Key(key.clone()),
            InputEvent::MouseButton { button, .. } => InputKind::MouseButton(*button),
            InputEvent::CursorPos(_) => InputKind::CursorPos,
        }
    }
}

pub enum InputEvent2D {
    CursorPos(Vec2),
    Scroll(Vec2),
}

pub struct Decompose<B, Axis> {
    binding: B,
    axis: Axis,
}

impl<B: Binding<Vec2>> Binding<f32> for Decompose<B, Axis2> {
    type Input = B::Input;

    fn apply(&self, input: &Self::Input) -> f32 {
        match self.axis {
            Axis2::X => self.binding.apply(input).x,
            Axis2::Y => self.binding.apply(input).y,
        }
    }

    fn binding(&self) -> InputKind {
        self.binding.binding()
    }
}

pub struct Compose<B, Axis> {
    binding: B,
    axis: Axis,
}

impl<B: Binding<Vec2>> Binding<Vec2> for Compose<B, Axis2> {
    type Input = B::Input;

    fn apply(&self, input: &Self::Input) -> Vec2 {
        match self.axis {
            Axis2::X => Vec2::X * self.binding.apply(input),
            Axis2::Y => Vec2::Y * self.binding.apply(input),
        }
    }

    fn binding(&self) -> InputKind {
        self.binding.binding()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum InputKind {
    Key(Key),
    MouseButton(MouseButton),
    CursorPos,
}

pub struct KeyBinding {
    key: Key<SmolStr>,
}

impl Binding<f32> for KeyBinding {
    type Input = InputEvent;

    fn apply(&self, input: &Self::Input) -> f32 {
        match input {
            InputEvent::Key { key, state } if key == &self.key => state.is_pressed() as i32 as f32,
            _ => panic!("Invalid input event"),
        }
    }

    fn binding(&self) -> InputKind {
        InputKind::Key(self.key.clone())
    }
}

pub struct MouseButtonBinding {
    button: MouseButton,
}

impl Binding<f32> for MouseButtonBinding {
    type Input = InputEvent;

    fn apply(&self, input: &Self::Input) -> f32 {
        match input {
            InputEvent::MouseButton { button, state } if button == &self.button => {
                state.is_pressed() as i32 as f32
            }
            _ => panic!("Invalid input event"),
        }
    }

    fn binding(&self) -> InputKind {
        InputKind::MouseButton(self.button)
    }
}

pub enum Axis2 {
    X,
    Y,
}

pub enum Axis3 {
    X,
    Y,
    Z,
}

pub trait Stimulus {
    const ZERO: Self;
    fn combine(&self, other: &Self) -> Self;
}

impl Stimulus for f32 {
    const ZERO: Self = 0.0;

    fn combine(&self, other: &Self) -> Self {
        self.max(*other)
    }
}

impl Stimulus for Vec2 {
    const ZERO: Self = Vec2::ZERO;

    fn combine(&self, other: &Self) -> Self {
        self.max(*other)
    }
}

impl Stimulus for Vec3 {
    const ZERO: Self = Vec3::ZERO;

    fn combine(&self, other: &Self) -> Self {
        self.max(*other)
    }
}

#[cfg(test)]
mod test {
    use winit::{event::ElementState, keyboard::Key};

    use crate::{ActivationMapping, InputEvent, InputState, KeyBinding};

    #[test]
    fn input_state() {
        let mut activation = ActivationMapping::new();
        activation
            .add(KeyBinding {
                key: Key::Character("A".into()),
            })
            .add(KeyBinding {
                key: Key::Character("B".into()),
            });

        activation.apply(InputEvent::Key {
            key: Key::Character("A".into()),
            state: ElementState::Pressed,
        });

        assert_eq!(activation.get_stimulus(), 1.0);

        activation.apply(InputEvent::Key {
            key: Key::Character("B".into()),
            state: ElementState::Pressed,
        });

        assert_eq!(activation.get_stimulus(), 1.0);

        activation.apply(InputEvent::Key {
            key: Key::Character("A".into()),
            state: ElementState::Released,
        });

        assert_eq!(activation.get_stimulus(), 1.0);
        activation.apply(InputEvent::Key {
            key: Key::Character("B".into()),
            state: ElementState::Released,
        });

        assert_eq!(activation.get_stimulus(), 0.0);
    }
}

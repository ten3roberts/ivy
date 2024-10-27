pub mod components;
pub mod error;
pub mod layer;
pub mod types;
mod vector;

use std::{collections::HashMap, mem, ops::Mul};

use flax::{component::ComponentValue, Component, EntityRef};
use glam::{vec2, Vec2, Vec3};

use types::{InputEvent, InputKind, KeyboardInput, MouseInput};
use winit::{
    event::MouseButton,
    keyboard::{Key, SmolStr},
};

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
                ActionKind::Scalar(_, mapping) => mapping.apply(event),
                ActionKind::Vector2(_, mapping) => mapping.apply(event),
                ActionKind::Vector3(_, mapping) => mapping.apply(event),
            }
        }
    }

    pub fn update(&mut self, entity: &EntityRef) -> anyhow::Result<()> {
        for activation in &mut self.activations {
            match activation {
                ActionKind::Scalar(target, m) => {
                    m.update(*target, entity)?;
                }
                ActionKind::Vector2(target, m) => {
                    m.update(*target, entity)?;
                }
                ActionKind::Vector3(target, m) => {
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
    Scalar(Component<f32>, Action<f32>),
    Vector2(Component<Vec2>, Action<Vec2>),
    Vector3(Component<Vec3>, Action<Vec3>),
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

pub struct Action<T> {
    bindings: HashMap<InputKind, Box<dyn Binding<T, Input = InputEvent>>>,
}

impl<T: ComponentValue + Stimulus> Action<T> {
    pub fn new() -> Self {
        Self {
            bindings: HashMap::new(),
        }
    }

    pub fn add(&mut self, action: impl 'static + Binding<T, Input = InputEvent>) -> &mut Self {
        self.bindings.insert(
            action.binding(),
            Box::new(action) as Box<dyn Binding<T, Input = InputEvent>>,
        );
        self
    }

    pub fn with_binding(mut self, action: impl 'static + Binding<T, Input = InputEvent>) -> Self {
        self.bindings.insert(
            action.binding(),
            Box::new(action) as Box<dyn Binding<T, Input = InputEvent>>,
        );
        self
    }

    fn apply(&mut self, event: &InputEvent) {
        if let Some(binding) = self.bindings.get_mut(&event.to_kind()) {
            binding.apply(event);
        }
    }

    fn get_stimulus(&mut self) -> T {
        self.bindings
            .values_mut()
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

pub trait Binding<T>: Send + Sync {
    type Input;

    fn apply(&mut self, input: &Self::Input);
    fn read(&mut self) -> T;
    fn binding(&self) -> InputKind;
}

pub struct Decompose<B, Axis> {
    binding: B,
    axis: Axis,
}

impl<B: Binding<Vec2>> Binding<f32> for Decompose<B, Axis2D> {
    type Input = B::Input;

    fn apply(&mut self, input: &Self::Input) {
        self.binding.apply(input);
    }

    fn read(&mut self) -> f32 {
        match self.axis {
            Axis2D::X => self.binding.read().x,
            Axis2D::Y => self.binding.read().y,
        }
    }

    fn binding(&self) -> InputKind {
        self.binding.binding()
    }
}

impl<B: Binding<Vec3>> Binding<f32> for Decompose<B, Axis3> {
    type Input = B::Input;

    fn apply(&mut self, input: &Self::Input) {
        self.binding.apply(input)
    }

    fn read(&mut self) -> f32 {
        match self.axis {
            Axis3::X => self.binding.read().x,
            Axis3::Y => self.binding.read().y,
            Axis3::Z => self.binding.read().z,
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

impl<B, Axis> Compose<B, Axis> {
    pub fn new(binding: B, axis: Axis) -> Self {
        Self { binding, axis }
    }
}

impl<B: Binding<f32>> Binding<Vec2> for Compose<B, Axis2D> {
    type Input = B::Input;

    fn apply(&mut self, input: &Self::Input) {
        self.binding.apply(input)
    }

    fn read(&mut self) -> Vec2 {
        match self.axis {
            Axis2D::X => Vec2::new(self.binding.read(), 0.0),
            Axis2D::Y => Vec2::new(0.0, self.binding.read()),
        }
    }

    fn binding(&self) -> InputKind {
        self.binding.binding()
    }
}

impl<B: Binding<f32>> Binding<Vec2> for Compose<B, Vec2> {
    type Input = B::Input;

    fn apply(&mut self, input: &Self::Input) {
        self.binding.apply(input)
    }

    fn read(&mut self) -> Vec2 {
        self.axis * self.binding.read()
    }

    fn binding(&self) -> InputKind {
        self.binding.binding()
    }
}

impl<B: Binding<f32>> Binding<Vec3> for Compose<B, Vec3> {
    type Input = B::Input;

    fn apply(&mut self, input: &Self::Input) {
        self.binding.apply(input)
    }

    fn read(&mut self) -> Vec3 {
        self.axis * self.binding.read()
    }

    fn binding(&self) -> InputKind {
        self.binding.binding()
    }
}

impl<B: Binding<f32>> Binding<Vec3> for Compose<B, Axis3> {
    type Input = B::Input;

    fn apply(&mut self, input: &Self::Input) {
        self.binding.apply(input)
    }

    fn read(&mut self) -> Vec3 {
        match self.axis {
            Axis3::X => Vec3::X * self.binding.read(),
            Axis3::Y => Vec3::Y * self.binding.read(),
            Axis3::Z => Vec3::Z * self.binding.read(),
        }
    }

    fn binding(&self) -> InputKind {
        self.binding.binding()
    }
}

pub struct Amplitude<B, T> {
    binding: B,
    amplitude: T,
}

impl<B: Binding<T>, T: Send + Sync + Copy + Mul<Output = T>> Binding<T> for Amplitude<B, T> {
    type Input = B::Input;

    fn apply(&mut self, input: &Self::Input) {
        self.binding.apply(input)
    }

    fn read(&mut self) -> T {
        self.binding.read() * self.amplitude
    }

    fn binding(&self) -> InputKind {
        self.binding.binding()
    }
}

pub struct KeyBinding {
    pressed: bool,
    key: Key<SmolStr>,
}

impl KeyBinding {
    pub fn new(key: Key<SmolStr>) -> Self {
        Self {
            key,
            pressed: false,
        }
    }
}

impl Binding<f32> for KeyBinding {
    type Input = InputEvent;

    fn apply(&mut self, input: &Self::Input) {
        match input {
            InputEvent::Keyboard(KeyboardInput { key, state, .. }) if key == &self.key => {
                self.pressed = state.is_pressed();
            }
            _ => panic!("Invalid input event"),
        }
    }

    fn read(&mut self) -> f32 {
        self.pressed as i32 as f32
    }

    fn binding(&self) -> InputKind {
        InputKind::Key(self.key.clone())
    }
}

pub struct MouseButtonBinding {
    pressed: bool,
    button: MouseButton,
}

impl MouseButtonBinding {
    pub fn new(button: MouseButton) -> Self {
        Self {
            button,
            pressed: false,
        }
    }
}

impl Binding<f32> for MouseButtonBinding {
    type Input = InputEvent;

    fn apply(&mut self, input: &Self::Input) {
        match input {
            InputEvent::MouseButton(MouseInput { button, state, .. }) if button == &self.button => {
                self.pressed = state.is_pressed();
            }
            _ => panic!("Invalid input event"),
        }
    }

    fn read(&mut self) -> f32 {
        self.pressed as i32 as f32
    }

    fn binding(&self) -> InputKind {
        InputKind::MouseButton(self.button)
    }
}

pub struct CursorMoveBinding {
    value: Vec2,
}

impl CursorMoveBinding {
    pub fn new() -> Self {
        Self { value: Vec2::ZERO }
    }
}

impl Default for CursorMoveBinding {
    fn default() -> Self {
        Self::new()
    }
}

impl Binding<Vec2> for CursorMoveBinding {
    type Input = InputEvent;

    fn apply(&mut self, input: &Self::Input) {
        match input {
            &InputEvent::CursorDelta(delta) => self.value += delta,
            _ => panic!("Invalid input event"),
        }
    }

    fn read(&mut self) -> Vec2 {
        mem::take(&mut self.value)
    }

    fn binding(&self) -> InputKind {
        InputKind::CursorDelta
    }
}

pub struct CursorPositionBinding {
    value: Vec2,
    normalized: bool,
}

impl CursorPositionBinding {
    pub fn new(normalized: bool) -> Self {
        Self {
            value: Vec2::ZERO,
            normalized,
        }
    }
}

impl Binding<Vec2> for CursorPositionBinding {
    type Input = InputEvent;

    fn apply(&mut self, input: &Self::Input) {
        match input {
            InputEvent::CursorMoved(v) if self.normalized => self.value = v.normalized_position,
            InputEvent::CursorMoved(v) => {
                self.value = vec2(v.absolute_position.x, v.absolute_position.y)
            }
            _ => panic!("Invalid input event"),
        }
    }

    fn read(&mut self) -> Vec2 {
        self.value
    }

    fn binding(&self) -> InputKind {
        InputKind::CursorMoved
    }
}

pub struct ScrollBinding {
    value: Vec2,
}

impl ScrollBinding {
    pub fn new() -> Self {
        Self { value: Vec2::ZERO }
    }
}

impl Default for ScrollBinding {
    fn default() -> Self {
        Self::new()
    }
}

impl Binding<Vec2> for ScrollBinding {
    type Input = InputEvent;

    fn apply(&mut self, input: &Self::Input) {
        match input {
            InputEvent::Scroll(delta) => self.value += delta.delta,
            _ => panic!("Invalid input event"),
        }
    }

    fn read(&mut self) -> Vec2 {
        mem::take(&mut self.value)
    }

    fn binding(&self) -> InputKind {
        InputKind::Scroll
    }
}

pub enum Axis2D {
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

pub trait BindingExt<V> {
    fn compose<T>(self, axis: T) -> Compose<Self, T>
    where
        Self: Sized,
    {
        Compose::new(self, axis)
    }

    fn decompose<T>(self, axis: T) -> Decompose<Self, T>
    where
        Self: Sized,
    {
        Decompose {
            binding: self,
            axis,
        }
    }

    fn amplitude<T>(self, amplitude: T) -> Amplitude<Self, T>
    where
        Self: Sized,
    {
        Amplitude {
            binding: self,
            amplitude,
        }
    }

    fn rising_edge(self) -> RisingEdge<Self>
    where
        Self: Sized,
    {
        RisingEdge {
            binding: self,
            prev_value: 0.0,
        }
    }
}

pub struct RisingEdge<T> {
    binding: T,
    prev_value: f32,
}

impl<T> Binding<f32> for RisingEdge<T>
where
    T: Binding<f32>,
{
    type Input = T::Input;

    fn apply(&mut self, input: &Self::Input) {
        self.binding.apply(input);
    }

    fn read(&mut self) -> f32 {
        let value = self.binding.read();
        if self.prev_value == 0.0 {
            self.prev_value = value;
            return value;
        }
        self.prev_value = value;

        0.0
    }

    fn binding(&self) -> InputKind {
        self.binding.binding()
    }
}

impl<T, V> BindingExt<V> for T where T: Binding<V> {}

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

        assert_eq!(activation.get_stimulus(), 1.0);

        activation.apply(&InputEvent::Keyboard(KeyboardInput {
            key: Key::Character("B".into()),
            state: ElementState::Pressed,
            modifiers: Default::default(),
            text: Default::default(),
        }));

        assert_eq!(activation.get_stimulus(), 1.0);

        activation.apply(&InputEvent::Keyboard(KeyboardInput {
            key: Key::Character("A".into()),
            state: ElementState::Released,
            modifiers: Default::default(),
            text: Default::default(),
        }));

        assert_eq!(activation.get_stimulus(), 1.0);
        activation.apply(&InputEvent::Keyboard(KeyboardInput {
            key: Key::Character("B".into()),
            state: ElementState::Released,
            modifiers: Default::default(),
            text: Default::default(),
        }));

        assert_eq!(activation.get_stimulus(), 0.0);
    }
}

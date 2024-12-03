use std::{mem, ops::Mul};

use glam::{vec2, IVec2, IVec3, Vec2, Vec3};
use winit::{
    event::MouseButton,
    keyboard::{Key, SmolStr},
};

use crate::types::{InputEvent, InputKind, KeyboardInput, MouseInput};

pub trait Binding<T>: Send + Sync {
    fn apply(&mut self, input: &InputEvent);
    fn read(&mut self) -> T;
    fn binding(&self) -> InputKind;
}

pub struct Decompose<B, Axis> {
    binding: B,
    axis: Axis,
}

impl<B: Binding<Vec2>> Binding<f32> for Decompose<B, Axis2D> {
    fn apply(&mut self, input: &InputEvent) {
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

impl<B: Binding<IVec2>> Binding<i32> for Decompose<B, Axis2D> {
    fn apply(&mut self, input: &InputEvent) {
        self.binding.apply(input);
    }

    fn read(&mut self) -> i32 {
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
    fn apply(&mut self, input: &InputEvent) {
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

impl<B: Binding<IVec3>> Binding<i32> for Decompose<B, Axis3> {
    fn apply(&mut self, input: &InputEvent) {
        self.binding.apply(input)
    }

    fn read(&mut self) -> i32 {
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

impl<B: Binding<f32>> Binding<Vec2> for Compose<B, Vec2> {
    fn apply(&mut self, input: &InputEvent) {
        self.binding.apply(input)
    }

    fn read(&mut self) -> Vec2 {
        self.axis * self.binding.read()
    }

    fn binding(&self) -> InputKind {
        self.binding.binding()
    }
}

impl<B: Binding<i32>> Binding<IVec2> for Compose<B, IVec2> {
    fn apply(&mut self, input: &InputEvent) {
        self.binding.apply(input)
    }

    fn read(&mut self) -> IVec2 {
        self.axis * self.binding.read()
    }

    fn binding(&self) -> InputKind {
        self.binding.binding()
    }
}

impl<B: Binding<f32>> Binding<Vec3> for Compose<B, Vec3> {
    fn apply(&mut self, input: &InputEvent) {
        self.binding.apply(input)
    }

    fn read(&mut self) -> Vec3 {
        self.axis * self.binding.read()
    }

    fn binding(&self) -> InputKind {
        self.binding.binding()
    }
}

impl<B: Binding<i32>> Binding<IVec3> for Compose<B, IVec3> {
    fn apply(&mut self, input: &InputEvent) {
        self.binding.apply(input)
    }

    fn read(&mut self) -> IVec3 {
        self.axis * self.binding.read()
    }

    fn binding(&self) -> InputKind {
        self.binding.binding()
    }
}

impl<B: Binding<f32>> Binding<Vec3> for Compose<B, Axis3> {
    fn apply(&mut self, input: &InputEvent) {
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

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Amplitude<B, T> {
    binding: B,
    amplitude: T,
}

impl<B: Binding<T>, T: Send + Sync + Copy + Mul<Output = T>> Binding<T> for Amplitude<B, T> {
    fn apply(&mut self, input: &InputEvent) {
        self.binding.apply(input)
    }

    fn read(&mut self) -> T {
        self.binding.read() * self.amplitude
    }

    fn binding(&self) -> InputKind {
        self.binding.binding()
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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

impl Binding<i32> for KeyBinding {
    fn apply(&mut self, input: &InputEvent) {
        match input {
            InputEvent::Keyboard(KeyboardInput { key, state, .. }) if key == &self.key => {
                self.pressed = state.is_pressed();
            }
            _ => panic!("Invalid input event"),
        }
    }

    fn read(&mut self) -> i32 {
        self.pressed as i32
    }

    fn binding(&self) -> InputKind {
        InputKind::Key(self.key.clone())
    }
}

pub struct Analog<T>(T);

impl<T> Binding<f32> for Analog<T>
where
    T: Binding<i32>,
{
    fn apply(&mut self, input: &InputEvent) {
        self.0.apply(input);
    }

    fn read(&mut self) -> f32 {
        self.0.read() as f32
    }

    fn binding(&self) -> InputKind {
        self.0.binding()
    }
}

impl<T> Binding<Vec2> for Analog<T>
where
    T: Binding<IVec2>,
{
    fn apply(&mut self, input: &InputEvent) {
        self.0.apply(input);
    }

    fn read(&mut self) -> Vec2 {
        self.0.read().as_vec2()
    }

    fn binding(&self) -> InputKind {
        self.0.binding()
    }
}

impl<T> Binding<Vec3> for Analog<T>
where
    T: Binding<IVec3>,
{
    fn apply(&mut self, input: &InputEvent) {
        self.0.apply(input);
    }

    fn read(&mut self) -> Vec3 {
        self.0.read().as_vec3()
    }

    fn binding(&self) -> InputKind {
        self.0.binding()
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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

impl Binding<i32> for MouseButtonBinding {
    fn apply(&mut self, input: &InputEvent) {
        match input {
            InputEvent::MouseButton(MouseInput { button, state, .. }) if button == &self.button => {
                self.pressed = state.is_pressed();
            }
            _ => panic!("Invalid input event"),
        }
    }

    fn read(&mut self) -> i32 {
        self.pressed as i32
    }

    fn binding(&self) -> InputKind {
        InputKind::MouseButton(self.button)
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
    fn apply(&mut self, input: &InputEvent) {
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

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
    fn apply(&mut self, input: &InputEvent) {
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

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
    fn apply(&mut self, input: &InputEvent) {
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
            prev_value: 0,
        }
    }

    fn analog(self) -> Analog<Self>
    where
        Self: Sized,
    {
        Analog(self)
    }
}

pub struct RisingEdge<T> {
    binding: T,
    prev_value: i32,
}

impl<T> Binding<i32> for RisingEdge<T>
where
    T: Binding<i32>,
{
    fn apply(&mut self, input: &InputEvent) {
        self.binding.apply(input);
    }

    fn read(&mut self) -> i32 {
        let value = self.binding.read();
        if self.prev_value == 0 {
            self.prev_value = value;
            return value;
        }
        self.prev_value = value;

        0
    }

    fn binding(&self) -> InputKind {
        self.binding.binding()
    }
}

impl<T, V> BindingExt<V> for T where T: Binding<V> {}

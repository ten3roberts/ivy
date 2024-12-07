use std::{mem, ops::Mul};

use glam::{vec2, IVec2, IVec3, Vec2, Vec3};
use winit::{
    event::MouseButton,
    keyboard::{Key, SmolStr},
};

use crate::types::{InputEvent, InputKind, KeyboardInput, MouseInput};

pub trait Binding: Send + Sync {
    type Value;
    fn apply(&mut self, input: &InputEvent);
    fn read(&mut self) -> Self::Value;
    fn binding(&self) -> InputKind;
}

pub trait Composable<Space> {
    type Output;
    fn compose(&self, axis: Space) -> Self::Output;
}

pub trait Decomposable<Space> {
    type Output;
    fn decompose(&self, axis: Space) -> Self::Output;
}

impl Composable<Axis2D> for i32 {
    type Output = IVec2;

    fn compose(&self, axis: Axis2D) -> Self::Output {
        match axis {
            Axis2D::X => IVec2::X * *self,
            Axis2D::Y => IVec2::Y * *self,
        }
    }
}

impl Composable<Axis3D> for i32 {
    type Output = IVec3;

    fn compose(&self, axis: Axis3D) -> Self::Output {
        match axis {
            Axis3D::X => IVec3::X * *self,
            Axis3D::Y => IVec3::Y * *self,
            Axis3D::Z => IVec3::Z * *self,
        }
    }
}

impl Composable<Axis2D> for f32 {
    type Output = Vec2;

    fn compose(&self, axis: Axis2D) -> Self::Output {
        match axis {
            Axis2D::X => Vec2::X * *self,
            Axis2D::Y => Vec2::Y * *self,
        }
    }
}

impl Composable<Axis3D> for f32 {
    type Output = Vec3;

    fn compose(&self, axis: Axis3D) -> Self::Output {
        match axis {
            Axis3D::X => Vec3::X * *self,
            Axis3D::Y => Vec3::Y * *self,
            Axis3D::Z => Vec3::Z * *self,
        }
    }
}

impl Decomposable<Axis2D> for Vec2 {
    type Output = f32;

    fn decompose(&self, axis: Axis2D) -> Self::Output {
        match axis {
            Axis2D::X => self.x,
            Axis2D::Y => self.y,
        }
    }
}

impl Decomposable<Axis2D> for IVec2 {
    type Output = i32;

    fn decompose(&self, axis: Axis2D) -> Self::Output {
        match axis {
            Axis2D::X => self.x,
            Axis2D::Y => self.y,
        }
    }
}

impl Decomposable<Axis3D> for Vec3 {
    type Output = f32;

    fn decompose(&self, axis: Axis3D) -> Self::Output {
        match axis {
            Axis3D::X => self.x,
            Axis3D::Y => self.y,
            Axis3D::Z => self.z,
        }
    }
}

impl Decomposable<Axis3D> for IVec3 {
    type Output = i32;

    fn decompose(&self, axis: Axis3D) -> Self::Output {
        match axis {
            Axis3D::X => self.x,
            Axis3D::Y => self.y,
            Axis3D::Z => self.z,
        }
    }
}

pub struct Decompose<B, Axis> {
    binding: B,
    axis: Axis,
}

impl<Space: Copy + Send + Sync, T: Decomposable<Space>, B: Binding<Value = T>> Binding
    for Decompose<B, Space>
{
    type Value = T::Output;

    fn apply(&mut self, input: &InputEvent) {
        self.binding.apply(input);
    }

    fn read(&mut self) -> Self::Value {
        self.binding.read().decompose(self.axis)
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

impl<Space: Copy + Send + Sync, T: Composable<Space>, B: Binding<Value = T>> Binding
    for Compose<B, Space>
{
    type Value = T::Output;

    fn apply(&mut self, input: &InputEvent) {
        self.binding.apply(input)
    }

    fn read(&mut self) -> Self::Value {
        self.binding.read().compose(self.axis)
    }

    fn binding(&self) -> InputKind {
        self.binding.binding()
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Amplitude<B, Rhs> {
    binding: B,
    amplitude: Rhs,
}

impl<B, T, Rhs> Binding for Amplitude<B, Rhs>
where
    B: Binding<Value = T>,
    T: Send + Sync + Copy + Mul<Rhs, Output = T>,
    Rhs: Copy + Send + Sync,
{
    type Value = T;

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

impl Binding for KeyBinding {
    type Value = bool;

    fn apply(&mut self, input: &InputEvent) {
        match input {
            InputEvent::Keyboard(KeyboardInput { key, state, .. }) if key == &self.key => {
                self.pressed = state.is_pressed();
            }
            _ => panic!("Invalid input event"),
        }
    }

    fn read(&mut self) -> bool {
        self.pressed
    }

    fn binding(&self) -> InputKind {
        InputKind::Key(self.key.clone())
    }
}

#[doc(hidden)]
pub trait AsAnalog {
    type Output;

    fn as_analog(self) -> Self::Output;
}

impl AsAnalog for bool {
    type Output = f32;

    fn as_analog(self) -> Self::Output {
        self as i32 as f32
    }
}

impl AsAnalog for i32 {
    type Output = f32;

    fn as_analog(self) -> Self::Output {
        self as f32
    }
}

pub struct Analog<T>(T);

impl<T, B> Binding for Analog<B>
where
    B: Binding<Value = T>,
    T: AsAnalog,
{
    type Value = T::Output;

    fn apply(&mut self, input: &InputEvent) {
        self.0.apply(input);
    }

    fn read(&mut self) -> Self::Value {
        self.0.read().as_analog()
    }

    fn binding(&self) -> InputKind {
        self.0.binding()
    }
}

pub struct Integral<T>(T);

impl<B> Binding for Integral<B>
where
    B: Binding<Value = bool>,
{
    type Value = i32;

    fn apply(&mut self, input: &InputEvent) {
        self.0.apply(input);
    }

    fn read(&mut self) -> Self::Value {
        self.0.read() as i32
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

impl Binding for MouseButtonBinding {
    type Value = bool;

    fn apply(&mut self, input: &InputEvent) {
        match input {
            InputEvent::MouseButton(MouseInput { button, state, .. }) if button == &self.button => {
                self.pressed = state.is_pressed();
            }
            _ => panic!("Invalid input event"),
        }
    }

    fn read(&mut self) -> bool {
        self.pressed
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

impl Binding for CursorMoveBinding {
    type Value = Vec2;

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

impl Binding for CursorPositionBinding {
    type Value = Vec2;

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

impl Binding for ScrollBinding {
    type Value = Vec2;

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

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Axis2D {
    X,
    Y,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Axis3D {
    X,
    Y,
    Z,
}

pub trait BindingExt
where
    Self: Binding,
{
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
            prev_value: false,
        }
    }

    fn analog(self) -> Analog<Self>
    where
        Self: Sized,
    {
        Analog(self)
    }

    fn integral(self) -> Integral<Self>
    where
        Self: Sized,
    {
        Integral(self)
    }
}

pub struct RisingEdge<T> {
    binding: T,
    prev_value: bool,
}

impl<T> Binding for RisingEdge<T>
where
    T: Binding<Value = bool>,
{
    type Value = bool;

    fn apply(&mut self, input: &InputEvent) {
        self.binding.apply(input);
    }

    fn read(&mut self) -> bool {
        let value = self.binding.read();
        if !self.prev_value {
            self.prev_value = value;
            return value;
        }
        self.prev_value = value;

        false
    }

    fn binding(&self) -> InputKind {
        self.binding.binding()
    }
}

impl<T> BindingExt for T where T: Binding {}

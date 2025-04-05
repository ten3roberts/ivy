use std::{mem, ops::Mul};

use glam::{vec2, IVec2, IVec3, Vec2, Vec3};
use winit::{
    event::MouseButton,
    keyboard::{Key, SmolStr},
};

use crate::{
    types::{InputEvent, InputKind, KeyboardInput, MouseInput},
    Stimulus,
};

pub trait Binding: Send + Sync {
    type Value;
    fn apply(&mut self, input: &InputEvent);
    fn read(&mut self) -> Self::Value;
    fn bindings(&self) -> Vec<InputKind>;
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

    fn bindings(&self) -> Vec<InputKind> {
        self.binding.bindings()
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

    fn bindings(&self) -> Vec<InputKind> {
        self.binding.bindings()
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

    fn bindings(&self) -> Vec<InputKind> {
        self.binding.bindings()
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Value<B, T> {
    binding: B,
    value: T,
}

impl<B, T> Binding for Value<B, T>
where
    B: Binding<Value = bool>,
    T: Send + Sync + Copy + Default,
{
    type Value = T;

    fn apply(&mut self, input: &InputEvent) {
        self.binding.apply(input)
    }

    fn read(&mut self) -> T {
        if self.binding.read() {
            self.value
        } else {
            T::default()
        }
    }

    fn bindings(&self) -> Vec<InputKind> {
        self.binding.bindings()
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct KeyBinding {
    pressed: bool,
    key: Key<SmolStr>,
}

impl KeyBinding {
    pub fn new(key: impl Into<Key<SmolStr>>) -> Self {
        Self {
            key: key.into(),
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
            _ => {}
        }
    }

    fn read(&mut self) -> bool {
        self.pressed
    }

    fn bindings(&self) -> Vec<InputKind> {
        vec![InputKind::Key(self.key.clone())]
    }
}

pub struct CompositeBinding<T, U> {
    binding: T,
    modifiers: Vec<U>,
}

impl<T, U> CompositeBinding<T, U> {
    pub fn new(binding: T, modifiers: impl IntoIterator<Item = U>) -> Self {
        Self {
            binding,
            modifiers: modifiers.into_iter().collect(),
        }
    }
}

impl<T, U> Binding for CompositeBinding<T, U>
where
    T: Binding,
    T::Value: std::fmt::Debug + Stimulus,
    U: Binding<Value = bool>,
{
    type Value = T::Value;

    fn apply(&mut self, input: &InputEvent) {
        self.binding.apply(input);
        for modifier in &mut self.modifiers {
            modifier.apply(input);
        }
    }

    fn read(&mut self) -> Self::Value {
        let value = self.binding.read();

        if self.modifiers.iter_mut().all(|v| v.read()) {
            value
        } else {
            T::Value::ZERO
        }
    }

    fn bindings(&self) -> Vec<InputKind> {
        self.binding
            .bindings()
            .into_iter()
            .chain(self.modifiers.iter().flat_map(|v| v.bindings()))
            .collect()
    }
}

#[doc(hidden)]
pub trait IntoAnalog {
    type Output;

    fn into_analog(self) -> Self::Output;
}

impl IntoAnalog for bool {
    type Output = f32;

    fn into_analog(self) -> Self::Output {
        self as i32 as f32
    }
}

impl IntoAnalog for i32 {
    type Output = f32;

    fn into_analog(self) -> Self::Output {
        self as f32
    }
}

#[doc(hidden)]
pub trait IntoIntegral {
    type Output;

    fn into_integral(self) -> Self::Output;
}

impl IntoIntegral for f32 {
    type Output = i32;

    fn into_integral(self) -> Self::Output {
        self as i32
    }
}

impl IntoIntegral for bool {
    type Output = i32;

    fn into_integral(self) -> Self::Output {
        self as i32
    }
}

pub struct Analog<T>(T);

impl<T, B> Binding for Analog<B>
where
    B: Binding<Value = T>,
    T: IntoAnalog,
{
    type Value = T::Output;

    fn apply(&mut self, input: &InputEvent) {
        self.0.apply(input);
    }

    fn read(&mut self) -> Self::Value {
        self.0.read().into_analog()
    }

    fn bindings(&self) -> Vec<InputKind> {
        self.0.bindings()
    }
}

pub struct Integral<T>(T);

impl<T, B> Binding for Integral<B>
where
    B: Binding<Value = T>,
    T: IntoIntegral,
{
    type Value = T::Output;

    fn apply(&mut self, input: &InputEvent) {
        self.0.apply(input);
    }

    fn read(&mut self) -> Self::Value {
        self.0.read().into_integral()
    }

    fn bindings(&self) -> Vec<InputKind> {
        self.0.bindings()
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
            _ => {}
        }
    }

    fn read(&mut self) -> bool {
        self.pressed
    }

    fn bindings(&self) -> Vec<InputKind> {
        vec![InputKind::MouseButton(self.button)]
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
        if let &InputEvent::CursorDelta(delta) = input {
            self.value += delta
        }
    }

    fn read(&mut self) -> Vec2 {
        mem::take(&mut self.value)
    }

    fn bindings(&self) -> Vec<InputKind> {
        vec![InputKind::CursorDelta]
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
            _ => {}
        }
    }

    fn read(&mut self) -> Vec2 {
        self.value
    }

    fn bindings(&self) -> Vec<InputKind> {
        vec![InputKind::CursorMoved]
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
        if let InputEvent::Scroll(delta) = input {
            self.value += delta.delta
        }
    }

    fn read(&mut self) -> Vec2 {
        mem::take(&mut self.value)
    }

    fn bindings(&self) -> Vec<InputKind> {
        vec![InputKind::Scroll]
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ScrollSteppedBinding {
    value: Vec2,
}

impl ScrollSteppedBinding {
    pub fn new() -> Self {
        Self { value: Vec2::ZERO }
    }
}

impl Default for ScrollSteppedBinding {
    fn default() -> Self {
        Self::new()
    }
}

impl Binding for ScrollSteppedBinding {
    type Value = Vec2;

    fn apply(&mut self, input: &InputEvent) {
        if let InputEvent::Scroll(delta) = input {
            self.value += delta.line_delta
        }
    }

    fn read(&mut self) -> Vec2 {
        mem::take(&mut self.value)
    }

    fn bindings(&self) -> Vec<InputKind> {
        vec![InputKind::Scroll]
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

    fn value<T>(self, value: T) -> Value<Self, T>
    where
        Self: Sized,
    {
        Value {
            binding: self,
            value,
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

    fn falling_edge(self) -> FallingEdge<Self>
    where
        Self: Sized,
    {
        FallingEdge {
            binding: self,
            prev_value: false,
        }
    }

    fn analog(self) -> Analog<Self>
    where
        Self: Sized,
        Self::Value: IntoAnalog,
    {
        Analog(self)
    }

    fn integral(self) -> Integral<Self>
    where
        Self: Sized,
        Self::Value: IntoIntegral,
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
        if !self.prev_value && value {
            self.prev_value = value;
            return true;
        }
        self.prev_value = value;

        false
    }

    fn bindings(&self) -> Vec<InputKind> {
        self.binding.bindings()
    }
}

pub struct FallingEdge<T> {
    binding: T,
    prev_value: bool,
}

impl<T> Binding for FallingEdge<T>
where
    T: Binding<Value = bool>,
{
    type Value = bool;

    fn apply(&mut self, input: &InputEvent) {
        self.binding.apply(input);
    }

    fn read(&mut self) -> bool {
        let value = self.binding.read();
        if self.prev_value && !value {
            self.prev_value = value;
            return true;
        }
        self.prev_value = value;

        false
    }

    fn bindings(&self) -> Vec<InputKind> {
        self.binding.bindings()
    }
}

impl<T> BindingExt for T where T: Binding {}

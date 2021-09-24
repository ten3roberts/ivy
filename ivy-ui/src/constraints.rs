use derive_more::{
    Add, AddAssign, AsRef, Deref, DerefMut, Div, DivAssign, From, Into, Mul, MulAssign, Sub,
    SubAssign,
};
use hecs::{Bundle, Query};
use ultraviolet::Vec2;

use crate::{Position2D, Size2D};

#[derive(
    Add,
    AddAssign,
    AsRef,
    Clone,
    Copy,
    Debug,
    Default,
    Deref,
    DerefMut,
    Div,
    DivAssign,
    From,
    Into,
    Mul,
    MulAssign,
    Sub,
    SubAssign,
)]
/// Constrains the position to an offset in pixels from parent origin.
pub struct AbsoluteOffset(pub Vec2);

impl From<AbsoluteOffset> for Position2D {
    fn from(p: AbsoluteOffset) -> Self {
        p.0.into()
    }
}

impl AbsoluteOffset {
    pub fn new(x: f32, y: f32) -> Self {
        Self(Vec2::new(x, y))
    }
}

#[derive(
    Add,
    AddAssign,
    AsRef,
    Clone,
    Copy,
    Debug,
    Default,
    Deref,
    DerefMut,
    Div,
    DivAssign,
    From,
    Into,
    Mul,
    MulAssign,
    Sub,
    SubAssign,
)]
/// Constrains the position of a widget relative to the bounds of the parent. -0.5 is left/bottom edge, and 0.5 is right/top edge.
pub struct RelativeOffset(pub Vec2);

impl From<RelativeOffset> for Position2D {
    fn from(p: RelativeOffset) -> Self {
        p.0.into()
    }
}

impl RelativeOffset {
    pub fn new(x: f32, y: f32) -> Self {
        Self(Vec2::new(x, y))
    }
}

#[derive(
    Add,
    AddAssign,
    AsRef,
    Clone,
    Copy,
    Debug,
    Default,
    Deref,
    DerefMut,
    Div,
    DivAssign,
    From,
    Into,
    Mul,
    MulAssign,
    Sub,
    SubAssign,
)]
/// Constrains the size of a widget to a multiple of the parent size. If paired
/// with [`Aspect`] width is ignored.
pub struct RelativeSize(pub Vec2);

impl From<RelativeSize> for Size2D {
    fn from(s: RelativeSize) -> Self {
        s.0.into()
    }
}

impl RelativeSize {
    pub fn new(x: f32, y: f32) -> Self {
        Self(Vec2::new(x, y))
    }
}

#[derive(
    Add,
    AddAssign,
    AsRef,
    Clone,
    Copy,
    Debug,
    Default,
    Deref,
    DerefMut,
    Div,
    DivAssign,
    From,
    Into,
    Mul,
    MulAssign,
    Sub,
    SubAssign,
)]
/// Constrains the size of a widget to pixels.
pub struct AbsoluteSize(pub Vec2);

impl From<AbsoluteSize> for Size2D {
    fn from(s: AbsoluteSize) -> Self {
        s.0.into()
    }
}

impl AbsoluteSize {
    pub fn new(x: f32, y: f32) -> Self {
        Self(Vec2::new(x, y))
    }
}
#[derive(
    Add,
    AddAssign,
    AsRef,
    Clone,
    Copy,
    Debug,
    Default,
    Deref,
    DerefMut,
    Div,
    DivAssign,
    From,
    Into,
    Mul,
    MulAssign,
    Sub,
    SubAssign,
)]
/// Constrains the widget width to a multiple of height.
pub struct Aspect(pub f32);

impl Aspect {
    pub fn new(aspect: f32) -> Self {
        Self(aspect)
    }
}

#[derive(Query)]
pub struct ConstraintQuery<'a> {
    pub rel_offset: Option<&'a RelativeOffset>,
    pub abs_offset: Option<&'a AbsoluteOffset>,
    pub rel_size: Option<&'a RelativeSize>,
    pub abs_size: Option<&'a AbsoluteSize>,
    pub aspect: Option<&'a Aspect>,
}

#[derive(Bundle)]
pub struct ConstraintBundle {
    pub rel_offset: RelativeOffset,
    pub abs_offset: AbsoluteOffset,
    pub rel_size: RelativeSize,
    pub abs_size: AbsoluteSize,
    pub aspect: Aspect,
}

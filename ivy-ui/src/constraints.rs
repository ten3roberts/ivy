// use derive_for::*;
use derive_for::*;
use derive_more::*;
use glam::Vec2;
use hecs::{Bundle, Query};
use ivy_base::{Position2D, Size2D};
#[cfg(feature = "serialize")]
use serde::{Deserialize, Serialize};

derive_for!(
    (
        Add, AddAssign, AsRef, Clone, Copy, Debug, Default, Deref, DerefMut, Div, DivAssign, From,
        Into, Mul, MulAssign, Sub, SubAssign,PartialEq,
    );
    /// Constrains the position to an offset in pixels from parent origin.
    #[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
    pub struct AbsoluteOffset(pub Vec2);
    #[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
    pub struct RelativeOffset(pub Vec2);
    /// Constrains the size of a widget to a multiple of the parent size. If paired
    /// with [`Aspect`] width is ignored.
    /// The aspect ratio of the parent is not preserved, as only the height will be
    /// considered from the parent. This ensures the window width doesn't stretch UI
    /// widgets.
    #[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
    pub struct RelativeSize(pub Vec2);
    /// Constrains the size of a widget to pixels.
    #[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
    pub struct AbsoluteSize(pub Vec2);

    /// Constrains the widget width to a multiple of height.
    /// If value is zero the aspect is unconstrained.
    #[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
    pub struct Aspect(pub f32);

    /// The offset of the origin from the center of the sprite.
    #[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
    pub struct Origin2D(pub Vec2);

    /// Margin for some widgets, like Text
    #[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
    pub struct Margin(pub Vec2);
);

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

impl Aspect {
    pub fn new(aspect: f32) -> Self {
        Self(aspect)
    }
}

impl Origin2D {
    pub fn new(x: f32, y: f32) -> Self {
        Self(Vec2::new(x, y))
    }

    pub fn lower_left() -> Self {
        Self::new(-1.0, -1.0)
    }

    pub fn lower_right() -> Self {
        Self::new(1.0, -1.0)
    }

    pub fn upper_right() -> Self {
        Self::new(1.0, 1.0)
    }

    pub fn upper_left() -> Self {
        Self::new(-1.0, 1.0)
    }
}

impl Margin {
    pub fn new(x: f32, y: f32) -> Self {
        Self(Vec2::new(x, y))
    }
}

#[derive(Query)]
pub struct ConstraintQuery<'a> {
    pub rel_offset: &'a RelativeOffset,
    pub abs_offset: &'a AbsoluteOffset,
    pub rel_size: &'a RelativeSize,
    pub abs_size: &'a AbsoluteSize,
    pub aspect: &'a Aspect,
    pub origin: &'a Origin2D,
}

#[derive(Bundle)]
pub struct ConstraintBundle {
    pub rel_offset: RelativeOffset,
    pub abs_offset: AbsoluteOffset,
    pub rel_size: RelativeSize,
    pub abs_size: AbsoluteSize,
    pub aspect: Aspect,
}

/// Trait for encompassing the different size constraints
pub trait UISize {
    fn calculate(&self, parent_size: Size2D) -> Size2D;
}

impl UISize for AbsoluteSize {
    fn calculate(&self, _: Size2D) -> Size2D {
        Size2D(**self)
    }
}

impl UISize for RelativeSize {
    fn calculate(&self, parent_size: Size2D) -> Size2D {
        Size2D(**self * *parent_size)
    }
}

/// Trait for encompassing the different offset constraints
pub trait UIOffset {
    fn calculate(&self, parent_size: Size2D) -> Position2D;
}

impl UIOffset for AbsoluteOffset {
    fn calculate(&self, _: Size2D) -> Position2D {
        Position2D(**self)
    }
}

impl UIOffset for RelativeOffset {
    fn calculate(&self, parent_size: Size2D) -> Position2D {
        Position2D(**self * *parent_size)
    }
}

impl<'a> ezy::Lerp<'a> for RelativeOffset {
    type Write = &'a mut RelativeOffset;

    fn lerp(write: Self::Write, start: &Self, end: &Self, t: f32) {
        *write = RelativeOffset(start.0.lerp(end.0, t))
    }
}

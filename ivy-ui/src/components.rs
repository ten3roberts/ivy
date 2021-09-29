use derive_for::*;
use derive_more::*;
pub use fontdue::layout::{HorizontalAlign, VerticalAlign};
use ultraviolet::Vec2;

derive_for!(
    (
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
        PartialEq,
    );
    pub struct Position2D(pub Vec2);
    pub struct Size2D(pub Vec2);
    #[derive(Eq, Hash)]
    /// The depth of the widget from the root.
    pub struct WidgetDepth(pub u32);
);

impl Position2D {
    pub fn new(x: f32, y: f32) -> Self {
        Self(Vec2::new(x, y))
    }
}

impl Size2D {
    pub fn new(x: f32, y: f32) -> Self {
        Self(Vec2::new(x, y))
    }
}

/// Marker type for UI and the UI hierarchy.
pub struct Widget;

#[derive(Clone, Copy, PartialEq)]
pub struct TextAlignment {
    pub horizontal: HorizontalAlign,
    pub vertical: VerticalAlign,
}

impl TextAlignment {
    pub fn new(horizontal: HorizontalAlign, vertical: VerticalAlign) -> Self {
        TextAlignment {
            horizontal,
            vertical,
        }
    }
}

impl Default for TextAlignment {
    fn default() -> Self {
        TextAlignment {
            horizontal: HorizontalAlign::Left,
            vertical: VerticalAlign::Top,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WrapStyle {
    /// Text flows outside bounds.
    Overflow,
    /// Text breaks at unicode word.
    Word,
    /// Text breaks at the overflowing character.
    Letter,
}

impl Default for WrapStyle {
    fn default() -> Self {
        Self::Word
    }
}

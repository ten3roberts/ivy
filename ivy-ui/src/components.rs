use derive_for::*;
use derive_more::*;
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
    )
    pub struct Position2D(pub Vec2);
    pub struct Size2D(pub Vec2);
    pub struct Foo {a: f32};
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

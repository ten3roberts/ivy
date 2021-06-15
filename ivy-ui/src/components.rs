use derive_more::*;
use ultraviolet::Vec2;

#[derive(
    Add,
    AddAssign,
    AsRef,
    Clone,
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
pub struct Position2D(Vec2);

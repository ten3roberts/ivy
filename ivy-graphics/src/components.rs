use derive_more::*;
use ultraviolet::Mat4;

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
)]
pub(crate) struct ModelMatrix(pub Mat4);

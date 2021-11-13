use derive_for::*;
use derive_more::*;
pub use fontdue::layout::{HorizontalAlign, VerticalAlign};

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
    #[derive(Hash, Eq, PartialOrd, Ord)]
    /// The depth of the widget from the root.
    #[repr(transparent)]
    pub struct WidgetDepth(pub u32);
);

/// Marker type specifying that this widget is interactive and will consume
/// click events and not forward them down. Does not neccessarily mean that the
/// widget will react to it.
/// The interactive widget doesn't neccessarily need to be a visible object,
/// which allows for transparent blockers in menus.
pub struct Interactive;
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

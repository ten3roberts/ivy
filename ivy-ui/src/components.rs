use crate::{constraints::*, Canvas, Font, Image, Text};
use derive_for::*;
use derive_more::*;
pub use fontdue::layout::{HorizontalAlign, VerticalAlign};
use glam::Vec2;
use hecs::{Bundle, DynamicBundleClone, Entity, EntityRef, World};
use ivy_base::{Color, Events, Position2D, Size2D, Visible};
use ivy_graphics::Camera;
use ivy_resources::Handle;
#[cfg(feature = "serialize")]
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

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
        Hash,
        Eq,
        PartialOrd,
        Ord,
    );
    /// The depth of the widget from the root.
    #[repr(transparent)]
    #[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
    pub struct WidgetDepth(pub u32);
);

/// Bundle for widgets.
/// Use further bundles for images and texts
#[derive(Bundle, Clone, Debug, Default, DynamicBundleClone)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub struct WidgetBundle {
    pub widget: Widget,
    pub visible: Visible,
    pub depth: WidgetDepth,
    pub abs_offset: AbsoluteOffset,
    pub rel_offset: RelativeOffset,
    pub abs_size: AbsoluteSize,
    pub rel_size: RelativeSize,
    pub origin: Origin2D,
    pub aspect: Aspect,
    pub pos: Position2D,
    pub size: Size2D,
}

impl WidgetBundle {
    pub fn new(
        abs_offset: AbsoluteOffset,
        rel_offset: RelativeOffset,
        abs_size: AbsoluteSize,
        rel_size: RelativeSize,
        origin: Origin2D,
        aspect: Aspect,
    ) -> Self {
        Self {
            widget: Widget,
            depth: WidgetDepth(0),
            abs_offset,
            rel_offset,
            abs_size,
            rel_size,
            origin,
            aspect,
            ..Default::default()
        }
    }
}

/// Bundle for widgets.
/// Use further bundles for images and texts
#[derive(Bundle, Clone, Debug, Default)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub struct CanvasBundle {
    pub widget: Widget,
    pub visible: Visible,
    pub depth: WidgetDepth,
    pub abs_offset: AbsoluteOffset,
    pub rel_offset: RelativeOffset,
    pub abs_size: AbsoluteSize,
    pub rel_size: RelativeSize,
    pos: Position2D,
    size: Size2D,
    pub origin: Origin2D,
    pub aspect: Aspect,
    pub canvas: Canvas,
    pub camera: Camera,
}

impl CanvasBundle {
    pub fn new<E: Into<Vec2>>(extent: E) -> Self {
        Self {
            abs_size: AbsoluteSize(extent.into()),
            ..Default::default()
        }
    }
}

#[derive(Default, Bundle, Debug, Clone, DynamicBundleClone)]
/// Specialize widget into an image
pub struct ImageBundle {
    pub image: Handle<Image>,
    pub color: Color,
}

impl ImageBundle {
    pub fn new(image: Handle<Image>, color: Color) -> Self {
        Self { image, color }
    }
}

#[derive(Default, Bundle, Debug, Clone, DynamicBundleClone)]
/// Specialize widget into text
#[records::record]
pub struct TextBundle {
    pub text: Text,
    pub font: Handle<Font>,
    pub color: Color,
    pub wrap: WrapStyle,
    pub align: Alignment,
    pub margin: Margin,
}

impl TextBundle {
    pub fn set_text<U: Into<Cow<'static, str>>>(&mut self, val: U) {
        self.text.set(val)
    }
}

/// Marker type specifying that this widget is interactive and will consume
/// click events and not forward them down. Does not neccessarily mean that the
/// widget will react to it.
/// The interactive widget doesn't neccessarily need to be a visible object,
/// which allows for transparent blockers in menus.
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub struct Interactive;
/// Marker type for UI and the UI hierarchy.
#[derive(Default, Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub struct Widget;

/// Marker type specifying that a widget should remain active even after the
/// mouse button was released. Release events will still be sent, but input will
/// continue to be absorbed and sent to the widget.
#[derive(Default, Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub struct Sticky;

#[cfg(feature = "serialize")]
#[derive(Serialize, Deserialize)]
#[serde(remote = "HorizontalAlign")]
enum HorizontalAlignDef {
    /// Aligns text to the left of the region defined by the max_width.
    Left,
    /// Aligns text to the center of the region defined by the max_width.
    Center,
    /// Aligns text to the right of the region defined by the max_width.
    Right,
}

#[cfg(feature = "serialize")]
#[derive(Serialize, Deserialize)]
#[serde(remote = "VerticalAlign")]
enum VerticalAlignDef {
    /// Aligns text to the top of the region defined by the max_height.
    Top,
    /// Aligns text to the middle of the region defined by the max_height.
    Middle,
    /// Aligns text to the bottom of the region defined by the max_height.
    Bottom,
}

#[derive(Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub struct Alignment {
    #[cfg_attr(feature = "serialize", serde(with = "HorizontalAlignDef"))]
    pub horizontal: HorizontalAlign,
    #[cfg_attr(feature = "serialize", serde(with = "VerticalAlignDef"))]
    pub vertical: VerticalAlign,
}

impl Alignment {
    pub fn new(horizontal: HorizontalAlign, vertical: VerticalAlign) -> Self {
        Alignment {
            horizontal,
            vertical,
        }
    }
}

impl std::fmt::Debug for Alignment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TextAlignment")
            .field(
                "horizontal",
                match self.horizontal {
                    HorizontalAlign::Left => &"Left",
                    HorizontalAlign::Center => &"Center",
                    HorizontalAlign::Right => &"Right",
                },
            )
            .field(
                "vertical",
                match self.vertical {
                    VerticalAlign::Top => &"Top",
                    VerticalAlign::Middle => &"Middle",
                    VerticalAlign::Bottom => &"Bottom",
                },
            )
            .finish()
    }
}

impl Default for Alignment {
    fn default() -> Self {
        Alignment {
            horizontal: HorizontalAlign::Left,
            vertical: VerticalAlign::Top,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
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

/// Provide a function to execute when a widget is clicked.
/// This can be used to send extra events when a specific widget is clicked.
#[derive(Clone)]
pub struct OnClick(pub fn(entity: EntityRef, &mut Events));

impl OnClick {
    pub fn execute(&self, world: &mut World, events: &mut Events, entity: Entity) {
        let entity = world.entity(entity).expect("Entity does not exist");
        (self.0)(entity, events)
    }
}

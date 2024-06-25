use crate::{Font, Image, InputField, Text, WidgetLayout};
use flax::{Entity, EntityBuilder, EntityRef, World};
pub use fontdue::layout::{HorizontalAlign, VerticalAlign};
use glam::Vec2;
use ivy_assets::Asset;
use ivy_core::{color, position, size, visible, Bundle, Color, Events, Visible};
use ivy_graphics::components::camera;
use std::borrow::Cow;

flax::component! {
    /// The depth of the widget in the tree
    pub widget_depth: u32,
    pub image: Asset<Image>,
    pub font: Asset<Font>,
    pub text: Text,
    pub input_field: InputField,
    pub interactive: (),
    pub widget: (),
    pub sticky: (),
    pub horizontal_align: HorizontalAlign,
    pub vertical_align: VerticalAlign,
    pub alignment: Alignment,
    pub wrap: WrapStyle,
    pub on_click: OnClick,

    // Constraints
    pub absolute_offset: Vec2,
    pub relative_offset: Vec2,
    pub relative_size: Vec2,
    pub absolute_size: Vec2,
    pub aspect: f32,
    pub origin: Vec2,
    pub margin: Vec2,

    pub widget_layout: WidgetLayout,

    pub children: Vec<Entity>,

    pub canvas: (),
}

/// Bundle for widgets.
/// Use further bundles for images and texts
#[derive(Clone, Debug, Default)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub struct WidgetBundle {
    pub abs_offset: Vec2,
    pub rel_offset: Vec2,
    pub abs_size: Vec2,
    pub rel_size: Vec2,
    pub origin: Vec2,
    pub aspect: f32,
}

impl WidgetBundle {
    pub fn new(
        abs_offset: Vec2,
        rel_offset: Vec2,
        abs_size: Vec2,
        rel_size: Vec2,
        origin: Vec2,
        aspect: f32,
    ) -> Self {
        Self {
            abs_offset,
            rel_offset,
            abs_size,
            rel_size,
            origin,
            aspect,
        }
    }
}

impl Bundle for WidgetBundle {
    fn mount(self, entity: &mut EntityBuilder) {
        entity
            .set_default(widget())
            .set_default(widget_depth())
            .set(visible(), Visible::Visible)
            .set(absolute_offset(), self.abs_offset)
            .set(relative_offset(), self.rel_offset)
            .set(absolute_size(), self.abs_size)
            .set(relative_size(), self.rel_size)
            .set(origin(), self.origin)
            .set(aspect(), self.aspect)
            .set_default(position())
            .set_default(size());
    }
}

/// Bundle for widgets.
/// Use further bundles for images and texts
#[derive(Clone, Debug, Default)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub struct CanvasBundle {
    widget: WidgetBundle,
    pub origin: Vec2,
    pub aspect: f32,
}

impl CanvasBundle {
    pub fn new<E: Into<Vec2>>(extent: E) -> Self {
        Self {
            widget: WidgetBundle {
                abs_size: extent.into(),
                ..Default::default()
            },
            ..Default::default()
        }
    }
}

impl Bundle for CanvasBundle {
    fn mount(self, entity: &mut EntityBuilder) {
        self.widget.mount(entity);
        entity
            .set(origin(), self.origin)
            .set(aspect(), self.aspect)
            .set_default(canvas())
            .set_default(camera());
    }
}

#[derive(Default, Debug, Clone)]
/// Specialize widget into an image
pub struct ImageBundle {
    pub image: Option<Asset<Image>>,
    pub color: Color,
}

impl ImageBundle {
    pub fn new(image: Asset<Image>, color: Color) -> Self {
        Self {
            image: Some(image),
            color,
        }
    }
}

impl Bundle for ImageBundle {
    fn mount(self, entity: &mut EntityBuilder) {
        entity.set_opt(image(), self.image).set(color(), self.color);
    }
}

#[derive(Debug, Clone)]
/// Specialize widget into text
pub struct TextBundle {
    pub text: Text,
    pub font: Asset<Font>,
    pub color: Color,
    pub wrap: WrapStyle,
    pub align: Alignment,
    pub margin: Vec2,
}

impl TextBundle {
    pub fn new(
        font: Asset<Font>,
        color: Color,
        wrap: WrapStyle,
        align: Alignment,
        margin: Vec2,
        text: Text,
    ) -> Self {
        Self {
            text,
            font,
            color,
            wrap,
            align,
            margin,
        }
    }

    pub fn set_text<U: Into<Cow<'static, str>>>(&mut self, val: U) {
        self.text.set(val)
    }
}

impl Bundle for TextBundle {
    fn mount(self, entity: &mut EntityBuilder) {
        entity
            .set(font(), self.font)
            .set(color(), self.color)
            .set(wrap(), self.wrap)
            .set(alignment(), self.align)
            .set(margin(), self.margin)
            .set(text(), self.text);
    }
}

#[cfg(feature = "serialize")]
#[derive(serde::Serialize, serde::Deserialize)]
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
#[derive(serde::Serialize, serde::Deserialize)]
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
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
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
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
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

use std::borrow::Cow;

use crate::Result;
use crate::Size2D;
use crate::TextAlignment;
use crate::WrapStyle;
use fontdue::layout::{self, GlyphPosition, Layout, TextStyle};
use ivy_graphics::NormalizedRect;
use ultraviolet::{Vec2, Vec3};

use crate::{Font, UIVertex};

pub struct Text {
    str: Cow<'static, str>,
    layout: Layout,
    dirty: bool,
    wrap: WrapStyle,
    old_bounds: Size2D,
}

impl Text {
    pub fn new<S: Into<Cow<'static, str>>>(wrap: WrapStyle, str: S) -> Self {
        let layout = Layout::new(fontdue::layout::CoordinateSystem::PositiveYUp);

        Self {
            str: str.into(),
            layout,
            dirty: true,
            wrap,
            old_bounds: Size2D(Vec2::zero()),
        }
    }

    pub fn str(&self) -> &str {
        self.str.as_ref()
    }

    /// Sets the texts value. If str is differerent the dirty flag will be set.
    pub fn set<S: Into<Cow<'static, str>>>(&mut self, str: S) {
        let str = str.into();

        if self.str != str {
            self.dirty = true;
        }

        self.str = str;
    }

    /// Returns the length of the internal text
    pub fn len(&self) -> usize {
        self.str.len()
    }

    /// Returns true if the text has been changed
    pub fn dirty(&self) -> bool {
        self.dirty
    }

    /// Sets the dirty flag. Only do this if you know what you are doing.
    pub fn set_dirty(&mut self, dirty: bool) {
        self.dirty = dirty;
    }

    /// Gets the last laid out bounds.
    pub fn old_bounds(&self) -> Size2D {
        self.old_bounds
    }

    /// Returns an iterator for layout out the text in quads
    pub fn layout<'a>(
        &mut self,
        font: &'a Font,
        bounds: Size2D,
        alignment: TextAlignment,
    ) -> Result<TextLayout<'a, std::slice::Iter<GlyphPosition>>> {
        dbg!(bounds);
        self.old_bounds = bounds;

        self.layout.reset(&fontdue::layout::LayoutSettings {
            x: 0.0,
            y: 0.0,
            max_width: if self.wrap == WrapStyle::Overflow {
                None
            } else {
                Some(bounds.x)
            },
            max_height: None,
            horizontal_align: alignment.horizontal,
            vertical_align: alignment.vertical,
            wrap_style: match self.wrap {
                WrapStyle::Word => layout::WrapStyle::Word,
                WrapStyle::Letter => layout::WrapStyle::Letter,
                _ => layout::WrapStyle::Letter,
            },
            wrap_hard_breaks: true,
        });

        self.layout
            .append(&[font.font()], &TextStyle::new(&self.str, font.size(), 0));

        Ok(TextLayout {
            font,
            glyphs: self.layout.glyphs().iter(),
        })
    }
}

/// An iterator for producing quads for a text string.
pub struct TextLayout<'a, I> {
    font: &'a Font,
    glyphs: I,
}

impl<'a, I: Iterator<Item = &'a GlyphPosition>> Iterator for TextLayout<'a, I> {
    type Item = [UIVertex; 4];

    fn next(&mut self) -> Option<Self::Item> {
        let glyph = self.glyphs.next()?;
        let key = glyph.key;

        let location = match self.font.get_normalized(key.glyph_index) {
            Ok(val) => val,
            Err(_) => (NormalizedRect::default()),
        };

        // let size = self.font.size();

        let width = (glyph.width as i32) as f32;

        let x = glyph.x as f32;

        let y = glyph.y as f32;

        let height = glyph.height as f32;

        let uv_base = Vec2::new(location.x as f32, location.y as f32);

        let vertices = [
            // Bottom Left
            UIVertex::new(
                Vec3::new(x, y, 0.0),
                uv_base + Vec2::new(0.0, location.height as f32),
            ),
            // Bottom Right
            UIVertex::new(
                Vec3::new(x + width, y, 0.0),
                uv_base + Vec2::new(location.width as f32, location.height as f32),
            ),
            // Top Right
            UIVertex::new(
                Vec3::new(x + width, y + height, 0.0),
                uv_base + Vec2::new(location.width as f32, 0.0),
            ),
            // Top Left
            UIVertex::new(Vec3::new(x, y + height, 0.0), uv_base),
        ];

        Some(vertices)
    }
}

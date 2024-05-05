use std::borrow::Cow;

use crate::Alignment;
use crate::Result;
use crate::WrapStyle;
use fontdue::layout::{self, GlyphPosition, Layout, TextStyle};
use glam::{Vec2, Vec3};
use ivy_graphics::NormalizedRect;

use crate::{Font, UIVertex};

pub struct Text {
    str: Cow<'static, str>,
    layout: Layout,
    dirty: bool,
    old_bounds: Vec2,
    old_wrap: WrapStyle,
    old_margin: Vec2,
}

impl Clone for Text {
    fn clone(&self) -> Self {
        Self::new(self.str.clone())
    }
}

impl std::fmt::Debug for Text {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Text").field("str", &self.str).finish()
    }
}

impl Default for Text {
    fn default() -> Self {
        Self::new("")
    }
}

impl Text {
    pub fn new<S: Into<Cow<'static, str>>>(str: S) -> Self {
        let layout = Layout::new(fontdue::layout::CoordinateSystem::PositiveYUp);

        Self {
            str: str.into(),
            layout,
            dirty: true,
            old_bounds: Vec2::default(),
            old_wrap: WrapStyle::Word,
            old_margin: Vec2::default(),
        }
    }

    pub fn val(&self) -> &str {
        self.str.as_ref()
    }

    pub fn val_mut(&mut self) -> &mut Cow<'static, str> {
        self.dirty = true;
        &mut self.str
    }

    /// Sets the texts value. If str is differerent the dirty flag will be set.
    pub fn set<S: Into<Cow<'static, str>>>(&mut self, val: S) {
        let str = val.into();

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
    pub(crate) fn old_bounds(&self) -> Vec2 {
        self.old_bounds
    }

    /// Gets the last laid out bounds.
    pub(crate) fn old_wrap(&self) -> WrapStyle {
        self.old_wrap
    }

    /// Get the text's old margin.
    pub(crate) fn old_margin(&self) -> Vec2 {
        self.old_margin
    }

    /// Returns an iterator for layout out the text in quads
    pub fn layout<'a>(
        &mut self,
        font: &'a Font,
        bounds: Vec2,
        wrap: WrapStyle,
        alignment: Alignment,
        margin: Vec2,
    ) -> Result<TextLayout<'a, std::slice::Iter<GlyphPosition>>> {
        self.old_bounds = bounds;
        self.old_wrap = wrap;

        self.layout.reset(&fontdue::layout::LayoutSettings {
            x: -bounds.x + margin.x,
            y: bounds.y - margin.y,
            max_width: if wrap == WrapStyle::Overflow {
                None
            } else {
                Some(bounds.x * 2.0 - margin.x)
            },
            max_height: Some(bounds.y * 2.0 + margin.y),
            horizontal_align: alignment.horizontal,
            vertical_align: alignment.vertical,
            wrap_style: match wrap {
                WrapStyle::Word => layout::WrapStyle::Word,
                WrapStyle::Letter => layout::WrapStyle::Letter,
                _ => layout::WrapStyle::Letter,
            },
            wrap_hard_breaks: true,
            line_height: font.size() + 2.0,
        });

        self.layout
            .append(&[font.font()], &TextStyle::new(&self.str, font.size(), 0));

        Ok(TextLayout {
            font,
            glyphs: self.layout.glyphs().iter(),
        })
    }

    pub fn append(&mut self, ch: char) {
        self.dirty = true;
        let s = self.str.to_mut();
        s.push(ch);
    }

    /// Removes the last word
    pub fn remove_back_word(&mut self) {
        let s = self.str.to_mut();

        if s.len() == 0 {
            return;
        }

        self.dirty = true;
        let mut first = true;

        while let Some(c) = s.pop() {
            if !first && !c.is_alphanumeric() {
                s.push(c);
                break;
            }
            first = false;
        }
    }

    /// Removes the last char
    pub fn remove_back(&mut self) {
        self.dirty = true;
        let s = self.str.to_mut();
        s.pop();
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

        let width = (glyph.width as i32) as f32;

        let x = glyph.x as f32; //+ self.bounds.x / 2.0;

        let y = glyph.y as f32; //- self.bounds.y / 2.0;

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

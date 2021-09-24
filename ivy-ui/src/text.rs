use std::{borrow::Cow, str::Chars};

use crate::Result;
use ultraviolet::{Vec2, Vec3};

use crate::{Font, UIVertex};

pub struct Text {
    str: Cow<'static, str>,
    dirty: bool,
}

impl Text {
    pub fn new<S: Into<Cow<'static, str>>>(str: S) -> Self {
        Self {
            str: str.into(),
            dirty: true,
        }
    }

    pub fn str(&self) -> &str {
        self.str.as_ref()
    }

    /// Sets the texts value. If str is differerent the dirty flag will be set.
    pub fn set_str<S: Into<Cow<'static, str>>>(&mut self, str: S) {
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

    /// Returns an iterator for layout out the text in quads
    pub fn layout<'a>(&self, font: &'a Font) -> Result<TextLayout<'a, Chars>> {
        Ok(TextLayout {
            font,
            glyphs: self.str.chars(),
            cursor: Vec2::zero(),
        })
    }
}

/// An iterator for producing quads for a text string.
pub struct TextLayout<'a, I> {
    font: &'a Font,
    glyphs: I,
    cursor: Vec2,
}

impl<'a, I: Iterator<Item = char>> Iterator for TextLayout<'a, I> {
    type Item = [UIVertex; 4];

    fn next(&mut self) -> Option<Self::Item> {
        let glyph = self.glyphs.next()?;

        let (metrics, location) = match self.font.get_normalized(glyph) {
            Ok(str) => str,
            Err(_) => return self.next(),
        };

        let size = self.font.size();

        let width = (metrics.width as i32) as f32 / size;

        let x = self.cursor.x as f32;

        let ymin = metrics.ymin as f32 / size;

        let height = metrics.height as f32 / size + ymin;

        let uv_base = Vec2::new(location.x as f32, location.y as f32);

        let vertices = [
            // Bottom Left
            UIVertex::new(
                Vec3::new(x, ymin, 0.0),
                uv_base + Vec2::new(0.0, location.height as f32),
            ),
            // Bottom Right
            UIVertex::new(
                Vec3::new(x + width, ymin, 0.0),
                uv_base + Vec2::new(location.width as f32, location.height as f32),
            ),
            // Top Right
            UIVertex::new(
                Vec3::new(x + width, height, 0.0),
                uv_base + Vec2::new(location.width as f32, 0.0),
            ),
            // Top Left
            UIVertex::new(Vec3::new(x, height, 0.0), uv_base),
        ];

        self.cursor += Vec2::new(metrics.advance_width / size, 0.0);

        Some(vertices)
    }
}

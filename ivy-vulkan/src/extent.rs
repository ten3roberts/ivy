use std::{
    fmt::Display,
    ops::{Add, Mul},
};
use ultraviolet::Vec2;

use ash::vk;

/// Represents a width and height.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Extent {
    pub width: u32,
    pub height: u32,
}

impl Extent {
    pub fn new(width: u32, height: u32) -> Extent {
        Self { width, height }
    }

    /// Returns the aspect ratio of the extent
    pub fn aspect(&self) -> f32 {
        self.width as f32 / self.height as f32
    }

    // Convert the extent into a float vector
    pub fn as_vec(&self) -> Vec2 {
        (*self).into()
    }
}

impl Display for Extent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({}, {})", self.width, self.height)
    }
}

impl Mul<u32> for Extent {
    type Output = Self;

    fn mul(self, rhs: u32) -> Self::Output {
        Self {
            width: self.width * rhs,
            height: self.height * rhs,
        }
    }
}

impl Add<u32> for Extent {
    type Output = Self;
    fn add(self, rhs: u32) -> Self::Output {
        Self {
            width: self.width + rhs,
            height: self.height + rhs,
        }
    }
}

// Conversions

impl Into<vk::Extent2D> for Extent {
    fn into(self) -> vk::Extent2D {
        vk::Extent2D {
            width: self.width,
            height: self.height,
        }
    }
}

impl Into<[u32; 2]> for Extent {
    fn into(self) -> [u32; 2] {
        [self.width, self.height]
    }
}

impl Into<(u32, u32)> for Extent {
    fn into(self) -> (u32, u32) {
        (self.width, self.height)
    }
}

impl From<vk::Extent2D> for Extent {
    fn from(v: vk::Extent2D) -> Self {
        Self {
            width: v.width,
            height: v.height,
        }
    }
}

impl From<[u32; 2]> for Extent {
    fn from(v: [u32; 2]) -> Self {
        Self {
            width: v[0],
            height: v[1],
        }
    }
}

impl From<(u32, u32)> for Extent {
    fn from(v: (u32, u32)) -> Self {
        Self {
            width: v.0,
            height: v.1,
        }
    }
}

impl From<[i32; 2]> for Extent {
    fn from(v: [i32; 2]) -> Self {
        Self {
            width: v[0] as u32,
            height: v[1] as u32,
        }
    }
}

impl From<(i32, i32)> for Extent {
    fn from(v: (i32, i32)) -> Self {
        Self {
            width: v.0 as u32,
            height: v.1 as u32,
        }
    }
}

impl From<[usize; 2]> for Extent {
    fn from(v: [usize; 2]) -> Self {
        Self {
            width: v[0] as u32,
            height: v[1] as u32,
        }
    }
}

impl From<(usize, usize)> for Extent {
    fn from(v: (usize, usize)) -> Self {
        Self {
            width: v.0 as u32,
            height: v.1 as u32,
        }
    }
}

// Float conversion

impl Into<[f32; 2]> for Extent {
    fn into(self) -> [f32; 2] {
        [self.width as f32, self.height as f32]
    }
}

impl Into<(f32, f32)> for Extent {
    fn into(self) -> (f32, f32) {
        (self.width as f32, self.height as f32)
    }
}

impl Into<Vec2> for Extent {
    fn into(self) -> Vec2 {
        Vec2::new(self.width as f32, self.height as f32)
    }
}

#[cfg(feature = "serialize")]
use serde::{Deserialize, Serialize};
use std::{
    fmt::Display,
    ops::{Add, Mul},
};
use glam::Vec2;

/// Represents a width and height.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
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
impl From<Extent> for [u32; 2] {
    fn from(val: Extent) -> Self {
        [val.width, val.height]
    }
}

impl From<Extent> for (u32, u32) {
    fn from(val: Extent) -> Self {
        (val.width, val.height)
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

impl From<Extent> for [f32; 2] {
    fn from(val: Extent) -> Self {
        [val.width as f32, val.height as f32]
    }
}

impl From<Extent> for (f32, f32) {
    fn from(val: Extent) -> Self {
        (val.width as f32, val.height as f32)
    }
}

impl From<Extent> for Vec2 {
    fn from(extent: Extent) -> Vec2 {
        Vec2::new(extent.width as f32, extent.height as f32)
    }
}

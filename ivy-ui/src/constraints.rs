// use derive_for::*;
use flax::{Component, EntityBuilder, Fetch};
use glam::Vec2;

use crate::{absolute_offset, absolute_size, aspect, origin, relative_offset, relative_size};

// impl Origin2D {
//     pub fn new(x: f32, y: f32) -> Self {
//         Self(Vec2::new(x, y))
//     }

//     pub fn lower_left() -> Self {
//         Self::new(-1.0, -1.0)
//     }

//     pub fn lower_right() -> Self {
//         Self::new(1.0, -1.0)
//     }

//     pub fn upper_right() -> Self {
//         Self::new(1.0, 1.0)
//     }

//     pub fn upper_left() -> Self {
//         Self::new(-1.0, 1.0)
//     }
// }

// impl Margin {
//     pub fn new(x: f32, y: f32) -> Self {
//         Self(Vec2::new(x, y))
//     }
// }

#[derive(Fetch)]
pub struct ConstraintQuery {
    pub rel_offset: Component<Vec2>,
    pub abs_offset: Component<Vec2>,
    pub rel_size: Component<Vec2>,
    pub abs_size: Component<Vec2>,
    pub aspect: Component<f32>,
    pub origin: Component<Vec2>,
}

impl ConstraintQuery {
    pub fn new() -> Self {
        Self {
            rel_offset: relative_offset(),
            abs_offset: absolute_offset(),
            rel_size: relative_size(),
            abs_size: absolute_size(),
            aspect: aspect(),
            origin: origin(),
        }
    }
}

impl Default for ConstraintQuery {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ConstraintBundle {
    pub rel_offset: Vec2,
    pub abs_offset: Vec2,
    pub rel_size: Vec2,
    pub abs_size: Vec2,
    pub aspect: f32,
}

impl ConstraintBundle {
    pub fn mount(&self, entity: &mut EntityBuilder) {
        entity
            .set(relative_offset(), self.rel_offset)
            .set(absolute_offset(), self.abs_offset)
            .set(relative_size(), self.rel_size)
            .set(absolute_size(), self.abs_size)
            .set(aspect(), self.aspect);
    }
}

pub(crate) fn calculate_relative(size: Vec2, parent_size: Vec2) -> Vec2 {
    size * parent_size
}

// /// Trait for encompassing the different size constraints
// pub trait UISize {
//     fn calculate(&self, parent_size: Size2D) -> Size2D;
// }

// impl UISize for Vec2 {
//     fn calculate(&self, _: Size2D) -> Size2D {
//         Size2D(**self)
//     }
// }

// impl UISize for RelativeSize {
//     fn calculate(&self, parent_size: Size2D) -> Size2D {
//         Size2D(**self * *parent_size)
//     }
// }

// /// Trait for encompassing the different offset constraints
// pub trait UIOffset {
//     fn calculate(&self, parent_size: Size2D) -> Position2D;
// }

// impl UIOffset for AbsoluteOffset {
//     fn calculate(&self, _: Size2D) -> Position2D {
//         Position2D(**self)
//     }
// }

// impl UIOffset for RelativeOffset {
//     fn calculate(&self, parent_size: Size2D) -> Position2D {
//         Position2D(**self * *parent_size)
//     }
// }

// impl<'a> ezy::Lerp<'a> for RelativeOffset {
//     type Write = &'a mut RelativeOffset;

//     fn lerp(write: Self::Write, start: &Self, end: &Self, t: f32) {
//         *write = RelativeOffset(start.0.lerp(end.0, t))
//     }
// }

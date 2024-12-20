//! This module contains physics utiliy functions

use glam::Vec3;
use rapier3d::math::DEFAULT_EPSILON;

/// Calculates the perpendicular velocity of a point rotating around origin.
pub fn velocity_at_point(p: Vec3, w: Vec3) -> Vec3 {
    if w.length_squared() < f32::EPSILON {
        Vec3::ZERO
    } else {
        -p.cross(w)
    }
}

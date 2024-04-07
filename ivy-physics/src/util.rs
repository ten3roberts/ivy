//! This module contains physics utiliy functions

use glam::Vec3;

/// Calculates the perpendicular velocity of a point rotating around origin.
pub fn point_vel(p: Vec3, w: Vec3) -> Vec3 {
    if w.length_squared() < std::f32::EPSILON {
        Vec3::ZERO
    } else {
        -p.cross(w)
    }
}

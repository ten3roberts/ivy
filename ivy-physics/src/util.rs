//! This module contains physics utiliy functions

use glam::Vec3;
use ivy_base::{components::AngularVelocity, Position};

/// Calculates the perpendicular velocity of a point rotating around origin.
pub fn point_vel(p: Position, w: AngularVelocity) -> Vec3 {
    if w.length_squared() < std::f32::EPSILON {
        Vec3::default()
    } else {
        -p.cross(*w)
    }
}

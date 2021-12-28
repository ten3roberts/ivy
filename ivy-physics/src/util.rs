//! This module contains physics utiliy functions

use ivy_base::{components::AngularVelocity, Position};
use ultraviolet::Vec3;

/// Calculates the perpendicular velocity of a point rotating around origin.
pub fn point_vel(p: Position, w: AngularVelocity) -> Vec3 {
    if w.mag_sq() < std::f32::EPSILON {
        Vec3::default()
    } else {
        -p.cross(*w)
    }
}

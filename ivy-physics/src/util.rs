//! This module contains physics utiliy functions

use ivy_base::{components::AngularVelocity, Position, Velocity};

/// Calculates the perpendicular velocity of a point rotating around origin.
pub fn point_vel(p: Position, w: AngularVelocity) -> Velocity {
    if w.length_squared() < std::f32::EPSILON {
        Velocity::default()
    } else {
        Velocity(-p.cross(*w))
    }
}

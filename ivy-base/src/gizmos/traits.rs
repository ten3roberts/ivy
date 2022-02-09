use glam::Vec3;

use crate::{Color, Line, Position, Sphere, DEFAULT_RADIUS};

use super::Gizmos;

pub trait DrawGizmos {
    /// Draw a set of gizmos using the current section
    fn draw_gizmos(&self, gizmos: &mut Gizmos, color: Color);
}

impl DrawGizmos for Vec3 {
    fn draw_gizmos(&self, gizmos: &mut Gizmos, color: Color) {
        gizmos.draw(
            Sphere {
                origin: *self,
                ..Default::default()
            },
            color,
        );
    }
}

impl DrawGizmos for Position {
    fn draw_gizmos(&self, gizmos: &mut Gizmos, color: Color) {
        gizmos.draw(
            Sphere {
                origin: **self,
                ..Default::default()
            },
            color,
        );
    }
}

impl DrawGizmos for [Vec3; 1] {
    fn draw_gizmos(&self, gizmos: &mut Gizmos, color: Color) {
        self[0].draw_gizmos(gizmos, color)
    }
}

impl DrawGizmos for [Vec3; 2] {
    fn draw_gizmos(&self, gizmos: &mut Gizmos, color: Color) {
        gizmos.draw(
            Line::from_points(self[0], self[1], DEFAULT_RADIUS, 1.0),
            color,
        )
    }
}

impl DrawGizmos for () {
    fn draw_gizmos(&self, _: &mut Gizmos, _: Color) {}
}

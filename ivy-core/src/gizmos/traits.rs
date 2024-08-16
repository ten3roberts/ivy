use glam::Vec3;

use crate::{Color, ColorExt, GizmosSection, Line, Sphere, DEFAULT_RADIUS};

pub trait DrawGizmos {
    /// Draw a set of gizmos using the current section
    fn draw_primitives(&self, gizmos: &mut GizmosSection);
}

impl<T: DrawGizmos> DrawGizmos for &T {
    fn draw_primitives(&self, gizmos: &mut GizmosSection) {
        (*self).draw_primitives(gizmos)
    }
}

impl DrawGizmos for Vec3 {
    fn draw_primitives(&self, gizmos: &mut GizmosSection) {
        gizmos.draw(Sphere {
            origin: *self,
            ..Default::default()
        });
    }
}

impl DrawGizmos for [Vec3; 1] {
    fn draw_primitives(&self, gizmos: &mut GizmosSection) {
        self[0].draw_primitives(gizmos)
    }
}

impl DrawGizmos for [Vec3; 2] {
    fn draw_primitives(&self, gizmos: &mut GizmosSection) {
        gizmos.draw(Line::from_points(
            self[0],
            self[1],
            DEFAULT_RADIUS,
            Color::blue(),
        ))
    }
}

impl DrawGizmos for () {
    fn draw_primitives(&self, _: &mut GizmosSection) {}
}

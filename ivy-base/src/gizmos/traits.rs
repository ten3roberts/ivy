use std::ops::DerefMut;

use ultraviolet::Vec3;

use crate::{Color, DEFAULT_RADIUS};

use super::Gizmos;

pub trait DrawGizmos {
    /// Draw a set of gizmos using the current section
    fn draw_gizmos<T: DerefMut<Target = Gizmos>>(&self, gizmos: T, color: Color);
}

impl DrawGizmos for Vec3 {
    fn draw_gizmos<T: DerefMut<Target = Gizmos>>(&self, mut gizmos: T, color: Color) {
        gizmos.push(crate::Gizmo::Sphere {
            origin: *self,
            color,
            radius: DEFAULT_RADIUS,
        })
    }
}

impl DrawGizmos for [Vec3; 1] {
    fn draw_gizmos<T: DerefMut<Target = Gizmos>>(&self, gizmos: T, color: Color) {
        self[0].draw_gizmos(gizmos, color)
    }
}

impl DrawGizmos for [Vec3; 2] {
    fn draw_gizmos<T: DerefMut<Target = Gizmos>>(&self, mut gizmos: T, color: Color) {
        gizmos.push(crate::Gizmo::Line {
            origin: self[0],
            color,
            dir: self[1] - self[0],
            radius: DEFAULT_RADIUS,
            corner_radius: 1.0,
        })
    }
}
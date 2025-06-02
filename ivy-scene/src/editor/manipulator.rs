use flax::Entity;
use glam::{Mat4, Quat, Vec3};
use ivy_core::{
    gizmos::{transforms::TranslateGizmo, DrawGizmos, GizmosSection},
    math::{Axis2D, Axis3D},
};
use ivy_physics::shapes::Plane;
use ordered_float::NotNan;
use tracing::info;

/// Allows manipulating an entity in world
pub struct EntityManipulator {
    interact_width: f32,
    position: Vec3,
    rotation: Quat,
}

impl EntityManipulator {
    pub fn new(position: Vec3, rotation: Quat) -> Self {
        Self {
            position,
            rotation,
            interact_width: 0.2,
        }
    }

    pub fn update(&mut self, position: Vec3, rotation: Quat) {
        self.position = position;
        self.rotation = rotation;
    }

    pub fn draw(&self, gizmos: &mut GizmosSection) {
        TranslateGizmo::new(Mat4::from_rotation_translation(
            self.rotation,
            self.position,
        ))
        .draw_primitives(gizmos);
    }

    pub fn intersect(&self, origin: Vec3, direction: Vec3) {
        assert!(direction.is_normalized());

        let hit = [Axis3D::X, Axis3D::Y, Axis3D::Z]
            .into_iter()
            .filter_map(|axis| {
                let dim = axis.to_vec3();
                if dim.dot(direction).abs() > 0.9 {
                    return None;
                }

                let up = direction.cross(dim).normalize();
                let normal = dim.cross(up).normalize();
                let plane = Plane::from_normal_and_point(normal, self.position);

                let hit = plane.intersect_ray(origin, direction)?;

                let hit_point = origin + direction * hit;

                let cross_dist = (hit_point - self.position).dot(up);

                let dist = (hit_point - self.position).dot(dim);

                if cross_dist > self.interact_width || dist > 1.0 {
                    return None;
                }

                // tracing::info!(?axis, ?hit, cross_dist);
                Some((axis, hit, dist))
            })
            .min_by_key(|v| NotNan::new(v.1).unwrap());

        let Some(hit) = hit else {
            return;
        };

        tracing::info!(?origin, ?direction, ?hit);
        // tracing::info!(?hit);
    }
}

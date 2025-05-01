use flax::{Component, Fetch};
use glam::{vec2, vec4, Mat4, Vec2, Vec3, Vec4Swizzles};
use ivy_core::components::world_transform;
use ivy_physics::rapier3d::prelude::Ray;
use ivy_wgpu::components::projection_matrix;

/// Returns a world ray from the given screen coordinates and camera.
///
/// Uses normalized cursor position
pub fn screen_to_world_ray(cursor_pos: Vec2, camera: CameraQueryItem) -> Ray {
    let cursor_pos = vec2(cursor_pos.x * 2.0 - 1.0, -(cursor_pos.y * 2.0 - 1.0));

    let ray_eye = camera.projection.inverse() * vec4(cursor_pos.x, cursor_pos.y, 1.0, 1.0);
    let ray_eye = vec4(ray_eye.x, ray_eye.y, -1.0, 0.0);

    let world_ray = (*camera.transform * ray_eye).xyz().normalize();

    let origin = camera.transform.transform_point3(Vec3::ZERO);

    Ray::new(origin.into(), world_ray.into())
}

#[derive(Fetch)]
pub struct CameraQuery {
    transform: Component<Mat4>,
    projection: Component<Mat4>,
}

impl CameraQuery {
    pub fn new() -> Self {
        Self {
            transform: world_transform(),
            projection: projection_matrix(),
        }
    }
}

impl Default for CameraQuery {
    fn default() -> Self {
        Self::new()
    }
}

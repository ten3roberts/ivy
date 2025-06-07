use std::convert::identity;

use flax::{Entity, EntityRef, World};
use glam::{Mat4, Quat, Vec3};
use ivy_core::{
    components::{self, position, rotation},
    gizmos::{
        transforms::{RotateGizmo, TranslateGizmo},
        DrawGizmos, GizmosSection,
    },
    math::Axis3D,
    palette::{IntoColor, Srgba, WithAlpha},
    Color, ColorExt,
};
use ivy_physics::{
    components::rigidbody_flags,
    shapes::{Plane, Ray},
    state::RigidBodyFlags,
};
use ordered_float::NotNan;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TransformMode {
    Translate,
    Rotate,
    // Scale, // TODO
}

#[derive(Debug)]
pub struct DragData {
    start_position: Vec3,
    start_rotation: Quat,
    new_position: Vec3,
    position_delta: Vec3,
    new_rotation: Quat,
    rotation_delta: Quat,
    hit: HitResult,
}

/// Allows manipulating an entity in world
pub struct TransformControls {
    arrow_width: f32,
    ring_width: f32,
    position: Vec3,
    rotation: Quat,
    drag_data: Option<DragData>,
    ring_radius: f32,
    dynamic_size: bool,
}

impl TransformControls {
    pub fn new(position: Vec3, rotation: Quat) -> Self {
        Self {
            position,
            rotation,
            arrow_width: 0.1,
            ring_width: 0.2,
            drag_data: None,
            ring_radius: 1.2,
            dynamic_size: false,
        }
    }

    pub fn update(&mut self, position: Vec3, rotation: Quat) {
        self.position = position;
        self.rotation = rotation;
    }

    pub fn draw(&self, gizmos: &mut GizmosSection, camera_ray: Ray) {
        let hit = self
            .drag_data
            .as_ref()
            .map(|v| v.hit)
            .or_else(|| self.intersect(camera_ray));

        let gizmo_scale = self.get_size(camera_ray);

        for mode in [TransformMode::Translate, TransformMode::Rotate] {
            let colors = self.get_handle_colors(
                hit.filter(|v| v.mode == mode).map(|v| v.axis),
                self.drag_data.is_some(),
            );
            match mode {
                TransformMode::Translate => {
                    TranslateGizmo::new(
                        Mat4::from_rotation_translation(self.rotation, self.position),
                        colors,
                    )
                    .with_size(gizmo_scale)
                    .draw_primitives(gizmos);
                }
                TransformMode::Rotate => {
                    RotateGizmo::new(
                        Mat4::from_rotation_translation(self.rotation, self.position),
                        colors,
                    )
                    .with_radius(self.ring_radius * gizmo_scale)
                    .with_tube_radius(0.02 * gizmo_scale)
                    .draw_primitives(gizmos);
                }
            }
        }
    }

    fn get_size(&self, camera_ray: Ray) -> f32 {
        if self.dynamic_size {
            let gizmo_distance = (self.position - camera_ray.origin()).length();
            gizmo_distance * 0.1
        } else {
            1.0
        }
    }

    fn get_handle_colors(&self, hovered_axis: Option<Axis3D>, dragging: bool) -> [Srgba; 3] {
        let mut colors = [Color::red(), Color::green(), Color::blue()];

        for (i, axis) in [Axis3D::X, Axis3D::Y, Axis3D::Z].iter().enumerate() {
            colors[i] = if hovered_axis == Some(*axis) {
                colors[i].with_alpha(0.8)
            } else if dragging {
                let mut hsla = colors[i].to_hsla();
                hsla.saturation = 0.2;
                hsla.alpha = 0.2;
                hsla.into_color()
            } else {
                let mut hsla = colors[i].to_hsla();
                hsla.saturation = 0.6;
                hsla.alpha = 0.6;
                hsla.into_color()
            };
        }

        colors
    }

    pub fn handle_mouse_down(&mut self, camera_ray: Ray) -> bool {
        if self.drag_data.is_some() {
            return false; // already dragging
        }

        if let Some(hit) = self.intersect(camera_ray) {
            self.drag_data = Some(DragData {
                start_position: self.position,
                start_rotation: self.rotation,
                hit,
                new_position: self.position,
                new_rotation: self.rotation,
                position_delta: Vec3::ZERO,
                rotation_delta: Quat::IDENTITY,
            });

            return true;
        }

        false
    }

    pub fn handle_mouse_move(&mut self, camera_ray: Ray) -> Option<&DragData> {
        if let Some(drag_data) = &mut self.drag_data {
            match drag_data.hit.mode {
                TransformMode::Rotate => {
                    let delta = Self::handle_rotate(camera_ray, drag_data);
                    drag_data.rotation_delta = delta;
                    drag_data.new_rotation = delta * drag_data.start_rotation;
                    self.rotation = drag_data.new_rotation;
                }
                TransformMode::Translate => {
                    let delta = Self::handle_translate(camera_ray, drag_data);
                    drag_data.position_delta = delta;
                    drag_data.new_position = drag_data.start_position + delta;
                    self.position = drag_data.new_position;
                }
            }
        }

        self.drag_data.as_ref()
    }

    fn handle_translate(camera_ray: Ray, drag_data: &DragData) -> Vec3 {
        let new_hit = drag_data
            .hit
            .plane
            .intersect_ray(camera_ray.origin(), camera_ray.direction());

        let Some(hit) = new_hit else {
            return Vec3::ZERO;
        };

        let hit_point = camera_ray.at(hit);

        let moved_dist = (hit_point - drag_data.start_position).dot(drag_data.hit.dim);

        drag_data.hit.dim * moved_dist - drag_data.hit.interact_point
    }

    fn handle_rotate(camera_ray: Ray, drag_data: &DragData) -> Quat {
        let new_hit = drag_data
            .hit
            .plane
            .intersect_ray(camera_ray.origin(), camera_ray.direction());

        let Some(hit) = new_hit else {
            return Quat::IDENTITY;
        };

        let hit_point = (camera_ray.at(hit) - drag_data.start_position).normalize();

        let quat = Quat::from_rotation_arc(
            drag_data.hit.interact_point.normalize(),
            hit_point.normalize(),
        );

        quat
    }

    pub fn handle_mouse_up(&mut self) {
        self.drag_data = None;
    }

    fn intersect(&self, camera_ray: Ray) -> Option<HitResult> {
        assert!(camera_ray.direction().is_normalized());

        let hit = [Axis3D::X, Axis3D::Y, Axis3D::Z]
            .into_iter()
            .flat_map(|axis| {
                [
                    self.hit_test_arrow(camera_ray, axis),
                    self.hit_test_ring(camera_ray, axis),
                ]
            })
            .filter_map(identity)
            .min_by_key(|v| NotNan::new(v.hit_distance).unwrap());

        return hit;
    }

    fn hit_test_ring(&self, camera_ray: Ray, axis: Axis3D) -> Option<HitResult> {
        let dim = self.rotation * axis.to_vec3();

        let normal = dim;
        let plane = Plane::from_normal_and_point(normal, self.position);

        let hit = plane.intersect_ray(camera_ray.origin(), camera_ray.direction())?;

        let hit_point = camera_ray.at(hit);

        let hit_radius = hit_point - self.position;

        let gizmo_scale = self.get_size(camera_ray);
        if (hit_radius.length() - self.ring_radius * gizmo_scale).abs()
            > self.ring_width * gizmo_scale
        {
            return None;
        }

        Some(HitResult {
            mode: TransformMode::Rotate,
            axis,
            hit_distance: hit,
            interact_point: hit_radius.normalize() * self.ring_radius,
            plane,
            dim,
        })
    }

    fn hit_test_arrow(&self, camera_ray: Ray, axis: Axis3D) -> Option<HitResult> {
        let dim = self.rotation * axis.to_vec3();

        if dim.dot(camera_ray.direction()).abs() > 0.9999 {
            return None;
        }

        let up = camera_ray.direction().cross(dim).normalize();
        let normal = dim.cross(up).normalize();
        let plane = Plane::from_normal_and_point(normal, self.position);

        let hit = plane.intersect_ray(camera_ray.origin(), camera_ray.direction())?;

        let hit_point = camera_ray.at(hit);

        let cross_dist = (hit_point - self.position).dot(up).abs();

        let dist = (hit_point - self.position).dot(dim);

        let gizmo_scale = self.get_size(camera_ray);

        if cross_dist > self.arrow_width * gizmo_scale || dist > gizmo_scale || dist < 0.0 {
            return None;
        }

        Some(HitResult {
            mode: TransformMode::Translate,
            axis,
            hit_distance: hit,
            interact_point: dim * dist,
            plane,
            dim,
        })
    }

    pub fn drag_data(&self) -> Option<&DragData> {
        self.drag_data.as_ref()
    }
}

#[derive(Clone, Copy, Debug)]
struct HitResult {
    pub mode: TransformMode,
    pub plane: Plane,
    pub axis: Axis3D,
    pub dim: Vec3,
    pub hit_distance: f32,
    pub interact_point: Vec3,
}

pub struct ManipulatedEntity {
    pub id: Entity,
    start_position: Vec3,
    start_rotation: Quat,
    position: Vec3,
    rotation: Quat,
    previous_flags: Option<RigidBodyFlags>,
}

impl ManipulatedEntity {
    pub fn new(
        id: Entity,
        position: Vec3,
        rotation: Quat,
        rigidbody_flags: Option<RigidBodyFlags>,
    ) -> Self {
        Self {
            id,
            start_position: position,
            start_rotation: rotation,
            position,
            rotation,
            previous_flags: rigidbody_flags,
        }
    }

    pub fn from_entity(entity: EntityRef) -> Self {
        let position = entity.get_copy(position()).unwrap_or_default();
        let rotation = entity.get_copy(rotation()).unwrap_or_default();
        let flags = entity.get_copy(rigidbody_flags()).ok();

        Self::new(entity.id(), position, rotation, flags)
    }
}

pub struct TransformController {
    pub entities: Vec<ManipulatedEntity>,
    pub manipulator: TransformControls,
    pub center: Vec3,
}

impl TransformController {
    pub fn new(entities: Vec<ManipulatedEntity>) -> Self {
        let center =
            entities.iter().map(|e| e.start_position).sum::<Vec3>() / entities.len().max(1) as f32;

        Self {
            manipulator: TransformControls::new(
                center,
                entities.last().map_or(Quat::IDENTITY, |e| e.start_rotation),
            ),
            entities,
            center,
        }
    }

    pub fn toggle_entity(&mut self, entity: ManipulatedEntity) {
        if let Some(index) = self.entities.iter().position(|v| v.id == entity.id) {
            self.entities.remove(index);
        } else {
            self.entities.push(entity);
        }

        self.update_center();
    }

    pub fn add_entity(&mut self, entity: ManipulatedEntity) {
        self.entities.retain(|v| v.id != entity.id);

        self.entities.push(entity);

        self.update_center();
    }

    fn update_center(&mut self) {
        self.center = self.entities.iter().map(|e| e.start_position).sum::<Vec3>()
            / self.entities.len() as f32;

        self.manipulator.update(
            self.center,
            self.entities
                .last()
                .map_or(Quat::IDENTITY, |e| e.start_rotation),
        );
    }

    pub fn clear_entities(&mut self) {
        self.entities.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }

    pub fn entities(&self) -> &[ManipulatedEntity] {
        &self.entities
    }

    pub fn update(&mut self, world: &World) {
        if self.manipulator.drag_data().is_some() {
            return; // don't update positions while dragging
        }

        for manipulated in &mut self.entities {
            if let Ok(entity) = world.entity(manipulated.id) {
                manipulated.start_position = entity.get_copy(position()).unwrap_or_default();
                manipulated.start_rotation = entity.get_copy(rotation()).unwrap_or_default();
            }
        }

        self.update_center();
    }

    pub fn draw(&self, gizmos: &mut GizmosSection, camera_ray: Ray) {
        if self.entities.is_empty() {
            return;
        }

        self.manipulator.draw(gizmos, camera_ray);
    }

    pub fn try_start_move(&mut self, camera_ray: Ray, world: &World) -> bool {
        if self.entities.is_empty() {
            return false;
        }

        if self.manipulator.handle_mouse_down(camera_ray) {
            for manipulated in &mut self.entities {
                if let Ok(entity) = world.entity(manipulated.id) {
                    manipulated.start_position = entity.get_copy(position()).unwrap_or_default();
                    manipulated.start_rotation = entity.get_copy(rotation()).unwrap_or_default();

                    // Store previous rigidbody flags if available
                    // manipulated.previous_flags = entity.get_copy(rigidbody_flags()).ok();
                    entity.update_dedup(rigidbody_flags(), RigidBodyFlags::new().enabled(false));
                }
            }

            return true;
        }
        false
    }

    pub fn handle_mouse_move(&mut self, camera_ray: Ray, world: &World) -> anyhow::Result<()> {
        if self.entities.is_empty() {
            return Ok(());
        }

        let drag = self.manipulator.handle_mouse_move(camera_ray);

        if let Some(drag) = drag {
            for manipulated in &mut self.entities {
                let Some(entity) = world.entity(manipulated.id).ok() else {
                    continue;
                };

                let start_transform = Mat4::from_rotation_translation(
                    manipulated.start_rotation,
                    manipulated.start_position,
                );

                let manipulator_start_transform =
                    Mat4::from_rotation_translation(drag.start_rotation, drag.start_position);

                let manipulator_new_transform =
                    Mat4::from_rotation_translation(drag.new_rotation, drag.new_position);

                let new_transform = manipulator_new_transform
                    * manipulator_start_transform.inverse()
                    * start_transform;

                let (_, rotation, position) = new_transform.to_scale_rotation_translation();

                manipulated.position = position;
                manipulated.rotation = rotation;

                entity.update_dedup(components::position(), position);
                entity.update_dedup(components::rotation(), rotation);
            }
        }

        Ok(())
    }

    pub fn finish_move(&mut self, world: &World) {
        if self.entities.is_empty() {
            return;
        }

        if self.manipulator.drag_data().is_some() {
            for manipulated in &mut self.entities {
                let Some(entity) = world.entity(manipulated.id).ok() else {
                    continue;
                };

                manipulated.start_position = manipulated.position;
                manipulated.start_rotation = manipulated.rotation;

                entity.update_dedup(components::position(), manipulated.position);
                entity.update_dedup(components::rotation(), manipulated.rotation);

                // Restore previous rigidbody flags if available
                if let Some(previous_flags) = manipulated.previous_flags {
                    entity.update_dedup(rigidbody_flags(), previous_flags);
                }
            }
        }

        self.manipulator.handle_mouse_up();
    }
}

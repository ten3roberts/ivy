use glam::{Mat4, Vec3};
use ivy_core::gizmos::{DrawGizmos, GizmosSection, Line};

mod cast;
pub use cast::*;

use crate::{epa, util::SupportPoint, Contact, Shape, Simplex, TransformedShape};

#[derive(Debug, Clone, Copy)]
pub struct Ray {
    pub(crate) origin: Vec3,
    pub(crate) dir: Vec3,
}

impl Ray {
    pub fn new(origin: Vec3, dir: Vec3) -> Self {
        Self {
            origin,
            dir: dir.normalize_or_zero(),
        }
    }

    pub fn support<T: Shape>(
        &self,
        collider: &T,
        transform: &Mat4,
        _transform_inv: &Mat4,
        dir: Vec3,
    ) -> SupportPoint {
        let a = TransformedShape::new(collider, *transform).support(dir);

        SupportPoint {
            p: a - self.origin,
            a,
            b: self.origin,
        }
    }

    /// Returns true if a shape intersects the ray
    pub fn intersect<T: Shape>(&self, collider: &T, transform: &Mat4) -> Option<Contact> {
        // Check if any point is behind ray

        let transform_inv = transform.inverse();
        let p = self.support(collider, transform, &transform_inv, -self.dir);
        if p.p.dot(self.dir) < 0.0 {
            return None;
        }

        // Get first support function in direction of separation
        // let dir = (a_pos - b_pos).normalized();
        let dir = Vec3::X;

        let a = self.support(collider, transform, &transform_inv, dir);

        let mut simplex = Simplex::Point([a]);

        let mut iterations = 0;
        while let Some(dir) = simplex.next_flat(self.dir) {
            if iterations > 1000 {
                tracing::error!("max iterations reached");
                return None;
            }
            iterations += 1;
            let dir = dir.normalize();

            // Get the next simplex
            let p = self.support(collider, transform, &transform_inv, dir);

            // New point was not past the origin
            // No collision
            if p.p.dot(dir) < 0.0 || !dir.is_normalized() {
                return None;
            }

            // p.pos += p.normalized() * 1.0;

            simplex.push(p);
        }

        // simplex.inflate(
        //     |dir| self.support(collider, &transform, &transform_inv, dir),
        //     self,
        // );

        // Collision found
        // Perform epa to find contact points
        Some(epa::epa_ray(
            |dir| self.support(collider, transform, &transform_inv, dir),
            simplex,
            self,
        ))
    }

    /// Get a reference to the ray's origin.
    pub fn origin(&self) -> Vec3 {
        self.origin
    }

    /// Get a reference to the ray's dir.
    pub fn dir(&self) -> Vec3 {
        self.dir
    }
}

impl DrawGizmos for Ray {
    fn draw_primitives(&self, gizmos: &mut GizmosSection) {
        gizmos.draw(Line {
            origin: self.origin,
            dir: self.dir,
            ..Default::default()
        })
    }
}

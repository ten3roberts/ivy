use hecs::{Entity, World};
use ivy_core::{Position, TransformMatrix};
use ultraviolet::{Mat4, Vec3};

use crate::{
    epa,
    util::{support, SupportPoint},
    Collider, CollisionPrimitive, Contact, Simplex,
};

pub struct Ray {
    origin: Vec3,
    dir: Vec3,
}

impl Ray {
    pub fn new(origin: Vec3, dir: Vec3) -> Self {
        Self {
            origin,
            dir: dir.normalized(),
        }
    }

    pub fn support<T: CollisionPrimitive>(
        &self,
        collider: &T,
        transform: &Mat4,
        transform_inv: &Mat4,
        dir: Vec3,
    ) -> SupportPoint {
        let a = support(&transform, &transform_inv, collider, dir);

        SupportPoint {
            support: a - self.origin,
            a,
            b: self.origin,
        }
    }

    /// Returns true if a shape intersects the ray
    pub fn intersects<T: CollisionPrimitive>(
        &self,
        collider: &T,
        transform: &TransformMatrix,
    ) -> Option<Contact> {
        // Check if any point is behind ray

        let transform_inv = transform.inversed();
        let p = self.support(collider, &transform, &transform_inv, -self.dir);
        if p.support.dot(self.dir) < 0.0 {
            return None;
        }

        // Get first support function in direction of separation
        // let dir = (a_pos - b_pos).normalized();
        let dir = Vec3::unit_x();

        let a = self.support(collider, &transform, &transform_inv, dir);

        let mut simplex = Simplex::Point([a]);

        while let Some(dir) = simplex.next_flat(self.dir) {
            let dir = dir.normalized();

            assert!((dir.mag() - 1.0 < 0.0001));
            // Get the next simplex
            let p = self.support(collider, &transform, &transform_inv, dir);

            // New point was not past the origin
            // No collision
            if p.support.dot(dir) < 0.0 {
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
            |dir| self.support(collider, &transform, &transform_inv, dir),
            simplex,
            self,
        ))
    }

    pub fn cast(&self, world: &World) -> Option<(Entity, Contact)> {
        world
            .query::<(&Position, &ivy_core::Rotation, &ivy_core::Scale, &Collider)>()
            .iter()
            .find_map(|(e, (pos, rot, scale, collider))| {
                let transform = TransformMatrix::new(*pos, *rot, *scale);

                if let Some(val) = self.intersects(collider, &transform) {
                    Some((e, val))
                } else {
                    None
                }
            })
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
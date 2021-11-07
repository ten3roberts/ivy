use std::ops::Deref;

use hecs::World;
use ivy_base::{DrawGizmos, Position, TransformMatrix, DEFAULT_RADIUS};
use ultraviolet::{Mat4, Vec3};

mod cast;
pub use cast::*;

use crate::{
    epa,
    query::TreeQuery,
    util::{support, SupportPoint},
    CollisionPrimitive, CollisionTree, Contact, Node, Simplex,
};

pub struct Ray {
    origin: Position,
    dir: Vec3,
}

impl Ray {
    pub fn new(origin: Position, dir: Vec3) -> Self {
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
        let a = support(transform, transform_inv, collider, dir);

        SupportPoint {
            support: a - *self.origin,
            a,
            b: *self.origin,
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
        let p = self.support(collider, transform, &transform_inv, -self.dir);
        if p.support.dot(self.dir) < 0.0 {
            return None;
        }

        // Get first support function in direction of separation
        // let dir = (a_pos - b_pos).normalized();
        let dir = Vec3::unit_x();

        let a = self.support(collider, transform, &transform_inv, dir);

        let mut simplex = Simplex::Point([a]);

        while let Some(dir) = simplex.next_flat(self.dir) {
            let dir = dir.normalized();

            assert!((dir.mag() - 1.0 < 0.0001));
            // Get the next simplex
            let p = self.support(collider, transform, &transform_inv, dir);

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
            |dir| self.support(collider, transform, &transform_inv, dir),
            simplex,
            self,
        ))
    }

    pub fn cast<'r, 'w, 't, T: Deref<Target = CollisionTree<N>>, N: Node>(
        &'r self,
        world: &'w World,
        tree: &'t T,
    ) -> TreeQuery<'t, N, RayCaster<'r, 'w>> {
        tree.query(RayCaster::new(self, world))
    }

    /// Get a reference to the ray's origin.
    pub fn origin(&self) -> Position {
        self.origin
    }

    /// Get a reference to the ray's dir.
    pub fn dir(&self) -> Vec3 {
        self.dir
    }
}

impl DrawGizmos for Ray {
    fn draw_gizmos<T: std::ops::DerefMut<Target = ivy_base::Gizmos>>(
        &self,
        mut gizmos: T,
        color: ivy_base::Color,
    ) {
        gizmos.push(ivy_base::Gizmo::Line {
            origin: *self.origin,
            color,
            dir: self.dir,
            radius: DEFAULT_RADIUS,
            corner_radius: 1.0,
        })
    }
}

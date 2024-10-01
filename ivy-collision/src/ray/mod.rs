use std::ops::Deref;

use flax::{Fetch, World};
use glam::{Mat4, Vec3};
use ivy_core::gizmos::{DrawGizmos, GizmosSection, Line};

mod cast;
pub use cast::*;
use ordered_float::OrderedFloat;

use crate::{
    epa, query::TreeQuery, util::SupportPoint, CollisionTree, Contact, Shape, Simplex,
    TransformedShape,
};

#[derive(Debug, Clone)]
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
    pub fn intersects<T: Shape>(&self, collider: &T, transform: &Mat4) -> Option<Contact> {
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

        while let Some(dir) = simplex.next_flat(self.dir) {
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

    /// Cast the ray into the world and returns the closest intersection
    pub fn cast_one<W>(&self, world: &World, tree: &CollisionTree) -> Option<RayIntersection> {
        tree.query(RayCaster::new(self, world, &()))
            .flatten()
            .min_by_key(|v| OrderedFloat(v.distance()))
    }

    pub fn cast<'a, Q>(
        &'a self,
        world: &'a World,
        tree: &'a CollisionTree,
        filter: &'a Q,
    ) -> TreeQuery<'a, RayCaster<'a, Q>> {
        tree.query(RayCaster::new(self, world, filter))
    }
    /// Cast the ray into the world and returns the closest intersection
    pub fn cast_one_with<'a, Q, T>(
        &'a self,
        world: &'a World,
        tree: &'a T,
        filter: &'a Q,
    ) -> Option<RayIntersection>
    where
        T: Deref<Target = CollisionTree>,
        Q: for<'x> Fetch<'x>,
    {
        tree.query(RayCaster::<Q>::new(self, world, filter))
            .flatten()
            .min_by_key(|v| OrderedFloat(v.distance()))
    }

    pub fn cast_with<'a, Q, T>(
        &'a self,
        world: &'a World,
        tree: &'a T,
        filter: &'a Q,
    ) -> TreeQuery<'a, RayCaster<'a, Q>>
    where
        T: Deref<Target = CollisionTree>,
    {
        tree.query(RayCaster::new(self, world, filter))
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

use std::ops::Deref;

use glam::{Mat4, Vec3};
use hecs::Query;
use hecs_schedule::GenericWorld;
use ivy_base::{DrawGizmos, Gizmos, Line, Position, TransformMatrix};

mod cast;
pub use cast::*;

use crate::{
    epa,
    query::TreeQuery,
    util::{support, SupportPoint},
    CollisionPrimitive, CollisionTree, CollisionTreeNode, Contact, Simplex,
};

pub struct Ray {
    pub(crate) origin: Position,
    pub(crate) dir: Vec3,
}

impl Ray {
    pub fn new(origin: Position, dir: Vec3) -> Self {
        Self {
            origin,
            dir: dir.normalize_or_zero(),
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
            support: *a - *self.origin,
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

        let transform_inv = transform.inverse();
        let p = self.support(collider, transform, &transform_inv, -self.dir);
        if p.support.dot(self.dir) < 0.0 {
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
            if p.support.dot(dir) < 0.0 || !dir.is_normalized() {
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
    pub fn cast_one<'r, 'w, 't, W, N>(
        &'r self,
        world: &'w W,
        tree: &'t CollisionTree<N>,
    ) -> Option<RayIntersection>
    where
        N: 'static + CollisionTreeNode,
        W: GenericWorld,
    {
        tree.query(RayCaster::<W, ()>::new(self, world))
            .flatten()
            .min()
    }

    pub fn cast<'r, 'w, 't, W, N>(
        &'r self,
        world: &'w W,
        tree: &'t CollisionTree<N>,
    ) -> TreeQuery<'t, N, RayCaster<'r, 'w, W, ()>>
    where
        N: CollisionTreeNode,
        W: GenericWorld,
    {
        tree.query(RayCaster::new(self, world))
    }
    /// Cast the ray into the world and returns the closest intersection
    pub fn cast_one_with<'r, 'w, 't, Q, T, W, N>(
        &'r self,
        world: &'w W,
        tree: &'t T,
    ) -> Option<RayIntersection>
    where
        T: Deref<Target = CollisionTree<N>>,
        N: 'static + CollisionTreeNode,
        Q: Query,
        W: GenericWorld,
    {
        tree.query(RayCaster::<W, Q>::new(self, world))
            .flatten()
            .min()
    }

    pub fn cast_with<'r, 'w, 't, Q, T, W, N>(
        &'r self,
        world: &'w W,
        tree: &'t T,
    ) -> TreeQuery<'t, N, RayCaster<'r, 'w, W, Q>>
    where
        T: Deref<Target = CollisionTree<N>>,
        N: 'static + CollisionTreeNode,
        W: GenericWorld,
    {
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
    fn draw_gizmos(&self, gizmos: &mut Gizmos, color: ivy_base::Color) {
        gizmos.draw(
            Line {
                origin: *self.origin,
                dir: self.dir,
                ..Default::default()
            },
            color,
        )
    }
}

use arrayvec::ArrayVec;
use hecs::Entity;
use hecs::World;
use ivy_core::Color;
use ivy_core::Events;
use ivy_core::Gizmo;
use ivy_core::Gizmos;
use slotmap::new_key_type;
use smallvec::Array;
use smallvec::SmallVec;
use std::mem;
use ultraviolet::Vec3;

use crate::intersect;
use crate::Collider;
use crate::Collision;

use super::node::Node;
use super::Nodes;
use super::Object;
use super::TreeMarker;

new_key_type!(
    pub struct NodeIndex;
);

impl NodeIndex {
    /// Returns the closest parent node that fully contains object. If object
    /// doesn't fit, the root node is returned.
    pub fn pop_up<const CAP: usize>(self, nodes: &mut Nodes<CAP>, object: &Object) -> NodeIndex {
        let node = &nodes[self];
        let parent = node.parent;
        if node.contains(object) {
            self
        } else {
            parent.pop_up(nodes, object)
        }
    }

    /// Returns the deepest node that fully contains object.
    pub fn push_down<const CAP: usize>(self, nodes: &Nodes<CAP>, object: &Object) -> Self {
        let node = &nodes[self];

        if let Some(child) = node.fits_child(nodes, &object) {
            child.push_down(nodes, object)
        } else {
            self
        }
    }

    pub fn remove<const CAP: usize>(self, nodes: &mut Nodes<CAP>, entity: Entity) {
        nodes[self].remove(entity)
    }

    /// Returns the child that fully contains object, if any.
    pub fn fits_child<const CAP: usize>(
        self,
        nodes: &Nodes<CAP>,
        object: &Object,
    ) -> Option<NodeIndex> {
        nodes[self]
            .children
            .into_iter()
            .flatten()
            .find(|val| nodes[*val].contains(object))
            .map(|val| val)
    }

    /// Inserts into node. Does not check if it is fully contained or if already
    /// in node.
    pub fn insert<const CAP: usize>(
        self,
        nodes: &mut Nodes<CAP>,
        object: Object,
        popped: &mut Vec<Object>,
    ) -> TreeMarker {
        let index = self.push_down(nodes, &object);
        let node = &mut nodes[index];

        if node.remaining_capacity() > 0 {
            node.push(object);
            TreeMarker { index, object }
        } else {
            eprintln!("Splitting");
            index.split(nodes, popped);
            index.insert(nodes, object, popped)
        }
    }

    /// Splits the node in half
    pub fn split<const CAP: usize>(self, nodes: &mut Nodes<CAP>, popped: &mut Vec<Object>) {
        // eprintln!("Splitting");
        let mut center = Vec3::zero();
        let mut max = Vec3::zero();
        let mut min = Vec3::zero();

        let node = &mut nodes[self];
        eprintln!("Children: {:?}", node.children);
        assert!(node.children.is_none());

        node.objects.iter().for_each(|val| {
            center += val.origin;
            max = max.max_by_component(val.origin);
            min = min.min_by_component(val.origin);
        });

        // let len = node.objects.len();
        // let center = center * (1.0 / len as f32);

        let width = (max - min).abs();

        let max = max_axis(width);

        let off = node.half_extents * max * 0.5;
        let origin = node.origin;

        let extents = node.half_extents - off;
        let a_origin = origin - off;
        let b_origin = origin + off;

        // let rel_center = (center - node.origin) * max;
        let rel_center = Vec3::zero();

        let a = Node::new(
            self,
            node.depth + 1,
            a_origin + rel_center,
            extents + rel_center,
        );
        let b = Node::new(
            self,
            node.depth + 1,
            b_origin + rel_center,
            extents - rel_center,
        );

        // Repartition nodes. Retain those that do not fit in any new leaf, and
        // push those that do to the popped list.
        let old = mem::replace(node.objects_mut(), ArrayVec::new());

        for obj in old {
            if a.contains(&obj) || b.contains(&obj) {
                popped.push(obj);
            } else {
                eprintln!("Pushing");
                node.push(obj)
            }
        }

        let a = nodes.insert(a);
        let b = nodes.insert(b);

        nodes[self].set_children([a, b]);
    }

    pub fn check_collisions<'a, T, const C: usize>(
        self,
        world: &World,
        events: &mut Events,
        nodes: &'a Nodes<C>,
        top_objects: &mut SmallVec<T>,
    ) -> Result<(), hecs::ComponentError>
    where
        T: Array<Item = &'a Object>,
    {
        let old_len = top_objects.len();
        let node = &nodes[self];
        let objects = &node.objects;

        // Check collision with objects above
        for i in 0..objects.len() {
            let a = objects[i];
            for b in objects[i + 1..].iter().chain(top_objects.iter().cloned()) {
                assert_ne!(a.entity, b.entity);

                // if true {
                if a.bound.overlaps(a.origin, b.bound, b.origin) {
                    let a_coll = world.get::<Collider>(a.entity)?;
                    let b_coll = world.get::<Collider>(b.entity)?;
                    // eprintln!("Possible intersection");
                    // *world.get_mut::<Color>(a.entity).unwrap() = Color::green();
                    // *world.get_mut::<Color>(b.entity).unwrap() = Color::green();
                    // Do full collision check

                    if let Some(intersection) =
                        intersect(&a.transform, &b.transform, &*a_coll, &*b_coll)
                    {
                        // eprintln!("Collision between {:?} and {:?}", a.entity, b.entity);
                        let collision = Collision {
                            a: a.entity,
                            b: b.entity,
                            intersection,
                        };

                        events.send(collision);
                    }
                }
            }
        }

        top_objects.extend(node.objects.iter());

        // Go deeper in tree
        node.children
            .iter()
            .flatten()
            .try_for_each(|val| val.check_collisions(world, events, nodes, top_objects))?;

        // Pop the stack
        unsafe { top_objects.set_len(old_len) };

        Ok(())
    }

    pub fn draw_gizmos<const CAP: usize>(
        self,
        world: &World,
        nodes: &Nodes<CAP>,
        depth: usize,
        gizmos: &mut Gizmos,
    ) {
        let node = &nodes[self];

        let color = Color::hsl(node.depth as f32 * 60.0, 1.0, 0.5);

        gizmos.push(Gizmo::Cube {
            origin: node.origin,
            color,
            half_extents: node.half_extents,
            radius: 0.02 + 0.001 * depth as f32,
            corner_radius: 1.0,
        });

        // for obj in &node.objects {
        //     let coll = world.get::<Collider>(obj.entity).unwrap();
        //     let scale = world.get::<Scale>(obj.entity).unwrap();
        //     gizmos.push(Gizmo::Sphere {
        //         origin: obj.transform.extract_translation(),
        //         color: Color::magenta(),
        //         radius: Sphere::enclose(&*coll, *scale).radius,
        //         corner_radius: 1.0,
        //     });
        // }

        node.children
            .into_iter()
            .flatten()
            .for_each(|val| val.draw_gizmos(world, nodes, depth + 1, gizmos))
    }
}

fn max_axis(val: Vec3) -> Vec3 {
    if val.x > val.y {
        if val.x > val.z {
            Vec3::new(1.0, 0.0, 0.0)
        } else {
            Vec3::new(0.0, 0.0, 1.0)
        }
    } else if val.y > val.z {
        Vec3::new(0.0, 1.0, 0.0)
    } else {
        Vec3::new(0.0, 0.0, 1.0)
    }
}

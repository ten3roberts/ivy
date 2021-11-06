use hecs::Entity;
use hecs::World;
use ivy_base::Color;
use ivy_base::Events;
use ivy_base::Gizmo;
use ivy_base::Gizmos;
use slotmap::new_key_type;
use smallvec::Array;
use smallvec::SmallVec;
use std::fmt::Debug;
use ultraviolet::Vec3;

use crate::intersect;
use crate::query::TreeQuery;
use crate::util::max_axis;
use crate::Collider;
use crate::Collision;
use crate::Cube;

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
    pub fn pop_up<T: Array<Item = Object>>(
        self,
        nodes: &mut Nodes<T>,
        object: &Object,
    ) -> NodeIndex {
        let node = &nodes[self];
        let parent = node.parent;
        if node.contains(object) {
            self
        } else {
            parent.pop_up(nodes, object)
        }
    }

    /// Returns the deepest node that fully contains object.
    pub fn push_down<T: Array<Item = Object>>(self, nodes: &Nodes<T>, object: &Object) -> Self {
        let node = &nodes[self];

        if let Some(child) = node.fits_child(nodes, &object) {
            child.push_down(nodes, object)
        } else {
            self
        }
    }

    /// Removes an entity from the node.
    /// Returns None if the entity didn't exists.
    /// This may happen if the entity was poepped from the node..
    pub fn remove<T: Array<Item = Object>>(
        self,
        nodes: &mut Nodes<T>,
        entity: Entity,
    ) -> Option<Object> {
        nodes[self].remove(entity)
    }

    /// Returns the child that fully contains object, if any.
    pub fn fits_child<T: Array<Item = Object>>(
        self,
        nodes: &Nodes<T>,
        object: &Object,
    ) -> Option<NodeIndex> {
        nodes[self]
            .children_iter()
            .find(|val| nodes[*val].contains(object))
    }

    /// Inserts into node. Does not check if it is fully contained or if already
    /// in node.
    pub fn insert<T: Array<Item = Object>>(
        self,
        nodes: &mut Nodes<T>,
        object: Object,
        popped: &mut Vec<Object>,
    ) -> TreeMarker {
        let index = self.push_down(nodes, &object);
        let node = &mut nodes[index];

        if node.full() && node.children.is_none() {
            index.split(nodes, popped);
            index.insert(nodes, object, popped)
        } else {
            node.push(object);
            TreeMarker { index, object }
        }

        // match node.remaining_capacity() {
        //     Some(0) => {
        //         index.split(nodes, popped);
        //         index.insert(nodes, object, popped)
        //     }
        //     None => unreachable!(),
        //     // Remaining capacoty and node is already split
        //     Some(_) | None => {
        //         node.push(object);
        //         TreeMarker { index, object }
        //     }
        // }
        // if let node.remaining_capacity().map(|val && node.children.is_none() {
        // } else {
        //     node.push(object);
        //     TreeMarker { index, object }
        // }

        // NOde is full and not split
        // if node.remaining_capacity() > 0 || node.children.is_some() {
        //     node.push(object);
        //     TreeMarker { index, object }
        // } else {
        //     eprintln!("Splitting");
        //     index.split(nodes, popped);
        //     index.insert(nodes, object, popped)
        // }
    }

    /// Splits the node in half
    pub fn split<T: Array<Item = Object>>(self, nodes: &mut Nodes<T>, popped: &mut Vec<Object>) {
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

        let width = (max - min).abs();

        let max = max_axis(width);

        let off = *node.bounds * max * 0.5;
        let origin = node.origin;

        let extents = *node.bounds - off;
        let a_origin = *origin - off;
        let b_origin = *origin + off;

        let a = Node::new(self, node.depth + 1, a_origin.into(), Cube::new(extents));
        let b = Node::new(self, node.depth + 1, b_origin.into(), Cube::new(extents));

        // Repartition nodes. Retain those that do not fit in any new leaf, and
        // push those that do to the popped list.

        node.clear().for_each(|val| popped.push(val));

        let a = nodes.insert(a);
        let b = nodes.insert(b);

        nodes[self].set_children([a, b]);
    }

    pub fn query<'a, T, V>(self, nodes: &'a Nodes<T>, visitor: V) -> TreeQuery<'a, T, V>
    where
        T: Array<Item = Object>,
    {
        TreeQuery::new(visitor, nodes, self)
    }

    pub fn check_collisions<'a, T, G>(
        self,
        world: &World,
        events: &mut Events,
        nodes: &'a Nodes<T>,
        top_objects: &mut SmallVec<G>,
    ) -> Result<(), hecs::ComponentError>
    where
        T: Array<Item = Object>,
        G: Array<Item = &'a Object>,
    {
        let old_len = top_objects.len();
        let node = &nodes[self];
        let objects = &node.objects;

        // Check collision with objects above
        for i in 0..objects.len() {
            let a = objects[i];
            for b in objects[i + 1..].iter().chain(top_objects.iter().cloned()) {
                // for b in objects[i + 1..].iter() {
                assert_ne!(a.entity, b.entity);

                // if true {
                if a.bound.overlaps(a.origin, b.bound, b.origin) {
                    let a_coll = world.get::<Collider>(a.entity)?;
                    let b_coll = world.get::<Collider>(b.entity)?;

                    // Do full collision check
                    if let Some(intersection) =
                        intersect(&a.transform, &b.transform, &*a_coll, &*b_coll)
                    {
                        let collision = Collision {
                            a: a.entity,
                            b: b.entity,
                            contact: intersection,
                        };

                        events.send(collision);
                    }
                }
            }
        }

        top_objects.extend(node.objects.iter());

        // Go deeper in tree
        node.children_iter()
            .try_for_each(|val| val.check_collisions(world, events, nodes, top_objects))?;

        // Pop the stack
        unsafe { top_objects.set_len(old_len) };

        Ok(())
    }

    pub fn draw_gizmos<T: Array<Item = Object>>(
        self,
        world: &World,
        nodes: &Nodes<T>,
        depth: usize,
        gizmos: &mut Gizmos,
    ) {
        let node = &nodes[self];

        let color = Color::hsl(
            node.depth as f32 * 60.0,
            1.0,
            if node.children.is_some() { 0.1 } else { 0.5 },
        );

        if node.object_count != 0 {
            gizmos.push(Gizmo::Cube {
                origin: *node.origin,
                color,
                half_extents: node.bounds.half_extents,
                radius: 0.02 + 0.001 * depth as f32,
            });
        }

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

        node.children_iter()
            .for_each(|val| val.draw_gizmos(world, nodes, depth + 1, gizmos))
    }
}

pub(crate) struct DebugNode<'a, T: Array<Item = Object>> {
    index: NodeIndex,
    nodes: &'a Nodes<T>,
}

impl<'a, T: Array<Item = Object>> DebugNode<'a, T> {
    pub(crate) fn new(index: NodeIndex, nodes: &'a Nodes<T>) -> Self {
        Self { index, nodes }
    }
}

impl<'a, T: Array<Item = Object>> Debug for DebugNode<'a, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let node = &self.nodes[self.index];
        let mut dbg = f.debug_struct("Node");
        dbg.field("object_count", &node.object_count);
        // dbg.field("objects", &node.objects.len());
        if let Some([a, b]) = node.children {
            let a = DebugNode::new(a, self.nodes);

            let b = DebugNode::new(b, self.nodes);

            dbg.field("left", &a);
            dbg.field("right", &b);
        }

        dbg.finish()
    }
}

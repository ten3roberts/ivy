use hecs::Entity;
use hecs::World;
use ivy_base::Events;
use ivy_base::Gizmos;
use slotmap::new_key_type;
use smallvec::Array;
use smallvec::SmallVec;
use std::fmt::Debug;

use crate::intersect;
use crate::query::TreeQuery;
use crate::Collider;
use crate::Collision;
use crate::Node;

use super::Nodes;
use super::Object;
use super::TreeMarker;

new_key_type!(
    pub struct NodeIndex;
);

impl NodeIndex {
    // /// Returns the closest parent node that fully contains object. If object
    // /// doesn't fit, the root node is returned.
    // pub fn pop_up<N: Node>(self, nodes: &mut Nodes<N>, object: &Object) -> NodeIndex {
    //     let node = &nodes[self];
    //     let parent = node.parent;
    //     if node.contains(object) {
    //         self
    //     } else {
    //         parent.pop_up(nodes, object)
    //     }
    // }

    pub fn fits_child<N: Node>(self, nodes: &Nodes<N>, object: &Object) -> Option<NodeIndex> {
        nodes[self]
            .children()
            .iter()
            .find(|index| nodes[**index].contains(object))
            .map(|val| *val)
    }

    /// Returns the deepest node that fully contains object.
    pub fn push_down<N: Node>(self, nodes: &Nodes<N>, object: &Object) -> Self {
        if let Some(child) = self.fits_child(nodes, object) {
            child.push_down(nodes, object)
        } else {
            self
        }
    }

    /// Removes an entity from the node.
    /// Returns None if the entity didn't exists.
    /// This may happen if the entity was poepped from the node..
    pub fn remove<N: Node>(self, nodes: &mut Nodes<N>, entity: Entity) -> Option<Object> {
        nodes[self].remove(entity)
    }

    /// Inserts into node. Does not check if it is fully contained or if already
    /// in node.
    pub fn insert<N: Node>(
        self,
        nodes: &mut Nodes<N>,
        object: Object,
        popped: &mut Vec<Object>,
    ) -> TreeMarker {
        let index = self.push_down(nodes, &object);
        let node = &mut nodes[index];

        if let Err(object) = node.try_add(object) {
            // It was not possible to insert the node
            // It was most likely full
            index.split(nodes, popped);
            index.insert(nodes, object, popped)
        } else {
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
    pub fn split<N: Node>(self, nodes: &mut Nodes<N>, popped: &mut Vec<Object>) {
        let output = nodes[self].split(popped);

        let indices = output
            .into_iter()
            .map(|val| nodes.insert(val))
            .collect::<SmallVec<[NodeIndex; 8]>>();

        nodes[self].set_children(&indices)
    }

    pub fn query<N, V>(self, nodes: &Nodes<N>, visitor: V) -> TreeQuery<N, V> {
        TreeQuery::new(visitor, nodes, self)
    }

    pub fn check_collisions<'a, N, G>(
        self,
        world: &World,
        events: &mut Events,
        nodes: &'a Nodes<N>,
        top_objects: &mut SmallVec<G>,
    ) -> Result<(), hecs::ComponentError>
    where
        N: Node,
        G: Array<Item = &'a Object>,
    {
        let old_len = top_objects.len();
        let node = &nodes[self];
        let objects = node.objects();

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

        top_objects.extend(node.objects().iter());

        // Go deeper in tree
        node.children()
            .iter()
            .try_for_each(|val| val.check_collisions(world, events, nodes, top_objects))?;

        // Pop the stack
        unsafe { top_objects.set_len(old_len) };

        Ok(())
    }

    pub fn draw_gizmos<N: Node>(
        self,
        world: &World,
        nodes: &Nodes<N>,
        depth: usize,
        gizmos: &mut Gizmos,
    ) {
        // let node = &nodes[self];

        // let color = Color::hsl(
        //     node.depth as f32 * 60.0,
        //     1.0,
        //     if node.children.is_some() { 0.1 } else { 0.5 },
        // );

        // if node.object_count != 0 {
        //     gizmos.push(Gizmo::Cube {
        //         origin: *node.origin,
        //         color,
        //         half_extents: node.bounds.half_extents,
        //         radius: 0.02 + 0.001 * depth as f32,
        //     });
        // }

        // // for obj in &node.objects {
        // //     let coll = world.get::<Collider>(obj.entity).unwrap();
        // //     let scale = world.get::<Scale>(obj.entity).unwrap();
        // //     gizmos.push(Gizmo::Sphere {
        // //         origin: obj.transform.extract_translation(),
        // //         color: Color::magenta(),
        // //         radius: Sphere::enclose(&*coll, *scale).radius,
        // //         corner_radius: 1.0,
        // //     });
        // // }

        // node.children_iter()
        //     .for_each(|val| val.draw_gizmos(world, nodes, depth + 1, gizmos))
    }
}

pub(crate) struct DebugNode<'a, N> {
    index: NodeIndex,
    nodes: &'a Nodes<N>,
}

impl<'a, N> DebugNode<'a, N> {
    pub(crate) fn new(index: NodeIndex, nodes: &'a Nodes<N>) -> Self {
        Self { index, nodes }
    }
}

impl<'a, N: Node> Debug for DebugNode<'a, N> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let node = &self.nodes[self.index];

        let mut children = f.debug_list();
        children.entries(node.children().iter().map(|val| {
            DebugNode::new(*val, self.nodes);
        }));

        let children = children.finish();
        let mut dbg = f.debug_struct("Node");
        dbg.field("object_count", &node.entity_count());
        // dbg.field("objects", &node.objects.len());

        dbg.field("children: ", &children);

        dbg.finish()
    }
}

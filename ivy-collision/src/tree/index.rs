use hecs::Entity;
use hecs::World;
use ivy_base::Color;
use ivy_base::DrawGizmos;
use ivy_base::Events;
use ivy_base::Gizmos;
use slotmap::new_key_type;
use slotmap::{Key, KeyData};
use smallvec::Array;
use smallvec::SmallVec;
use std::fmt::Debug;
use std::hash::Hash;
use std::marker::PhantomData;
use std::ops::DerefMut;

use crate::intersect;
use crate::query::TreeQuery;
use crate::Collider;
use crate::Collision;
use crate::Node;

use super::Nodes;
use super::Object;
use super::TreeMarker;

pub struct NodeIndex<T>(KeyData, PhantomData<T>);

impl<T> NodeIndex<T> {
    /// Creates a new handle that is always invalid and distinct from any non-null
    /// handle. A null key can only be created through this method (or default
    /// initialization of handles made with `new_key_type!`, which calls this
    /// method).
    ///
    /// A null handle is always invalid, but an invalid key (that is, a key that
    /// has been removed from the slot map) does not become a null handle. A null
    /// is safe to use with any safe method of any slot map instance.
    pub fn null() -> Self {
        Key::null()
    }

    /// Checks if a handle is null. There is only a single null key, that is
    /// `a.is_null() && b.is_null()` implies `a == b`.
    pub fn is_null(&self) -> bool {
        Key::is_null(self)
    }

    /// Removes the type from a handle, easier storage without using dynamic
    /// dispatch
    pub fn into_untyped(&self) -> NodeIndexUntyped {
        NodeIndexUntyped(self.data())
    }

    /// Converts an untyped handle into a typed handle.
    /// Behaviour is undefined if handle is converted back to the wrong type.
    /// Use with care.
    pub fn from_untyped(handle: NodeIndexUntyped) -> NodeIndex<T> {
        Self(handle.data(), PhantomData)
    }
}

new_key_type!(
    pub struct NodeIndexUntyped;
);

unsafe impl<T> Key for NodeIndex<T> {
    fn data(&self) -> KeyData {
        self.0
    }
}

impl<T> PartialOrd for NodeIndex<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.0.cmp(&other.0))
    }
}

impl<T> Ord for NodeIndex<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.cmp(&other.0)
    }
}

impl<T> Default for NodeIndex<T> {
    fn default() -> Self {
        Self(KeyData::default(), PhantomData)
    }
}

impl<T> std::fmt::Debug for NodeIndex<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

impl<T> Clone for NodeIndex<T> {
    fn clone(&self) -> Self {
        Self(self.0, PhantomData)
    }
}

impl<T> Copy for NodeIndex<T> {}

impl<T> PartialEq for NodeIndex<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<T> Eq for NodeIndex<T> {}

impl<T> Hash for NodeIndex<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state)
    }
}

impl<T> From<KeyData> for NodeIndex<T> {
    fn from(k: KeyData) -> Self {
        Self(k, PhantomData)
    }
}

impl<N: Node> NodeIndex<N> {
    // /// Returns the closest parent node that fully contains object. If object
    // /// doesn't fit, the root node is returned.
    // pub fn pop_up(self, nodes: &mut Nodes<N>, object: &Object) -> NodeIndex {
    //     let node = &nodes[self];
    //     let parent = node.parent;
    //     if node.contains(object) {
    //         self
    //     } else {
    //         parent.pop_up(nodes, object)
    //     }
    // }

    pub fn fits_child(self, nodes: &Nodes<N>, object: &Object) -> Option<NodeIndex<N>> {
        nodes[self]
            .children()
            .iter()
            .find(|index| nodes[**index].contains(object))
            .map(|val| *val)
    }

    /// Returns the deepest node that fully contains object.
    pub fn push_down(self, nodes: &Nodes<N>, object: &Object) -> Self {
        if let Some(child) = self.fits_child(nodes, object) {
            child.push_down(nodes, object)
        } else {
            self
        }
    }

    /// Removes an entity from the node.
    /// Returns None if the entity didn't exists.
    /// This may happen if the entity was poepped from the node..
    pub fn remove(self, nodes: &mut Nodes<N>, entity: Entity) -> Option<Object> {
        nodes[self].remove(entity)
    }

    /// Inserts into node. Does not check if it is fully contained or if already
    /// in node.
    pub fn insert(
        self,
        nodes: &mut Nodes<N>,
        object: Object,
        popped: &mut Vec<Object>,
    ) -> TreeMarker<N> {
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
    pub fn split(self, nodes: &mut Nodes<N>, popped: &mut Vec<Object>) {
        let output = nodes[self].split(popped);

        let indices = output
            .into_iter()
            .map(|val| nodes.insert(val))
            .collect::<SmallVec<[NodeIndex<N>; 8]>>();

        nodes[self].set_children(&indices)
    }

    pub fn query<V>(self, nodes: &Nodes<N>, visitor: V) -> TreeQuery<N, V> {
        TreeQuery::new(visitor, nodes, self)
    }

    pub fn check_collisions<'a, G>(
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
}

impl<N: Node + DrawGizmos> NodeIndex<N> {
    pub fn draw_gizmos_recursive(self, nodes: &Nodes<N>, mut gizmos: &mut Gizmos, color: Color) {
        nodes[self].draw_gizmos(gizmos.deref_mut(), color);

        for val in nodes[self].children().iter() {
            val.draw_gizmos_recursive(nodes, gizmos, color)
        }
    }
}

pub(crate) struct DebugNode<'a, N> {
    index: NodeIndex<N>,
    nodes: &'a Nodes<N>,
}

impl<'a, N> DebugNode<'a, N> {
    pub(crate) fn new(index: NodeIndex<N>, nodes: &'a Nodes<N>) -> Self {
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

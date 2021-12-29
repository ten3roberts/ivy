use std::any;

use flume::{Receiver, Sender};
use hecs::{Entity, Query};
use hecs_schedule::{CommandBuffer, GenericWorld, SubWorld, Write};
use ivy_base::components::Velocity;
use ivy_base::{
    Color, DrawGizmos, Events, Gizmos, Static, TransformMatrix, TransformQuery, Trigger, Visible,
};
use ivy_resources::{DefaultResource, DefaultResourceMut};
use records::record;
use slotmap::{new_key_type, SlotMap};
use smallvec::Array;

use crate::{BoundingBox, Collider, CollisionPrimitive};

mod binary_node;
mod bvh;
mod index;
mod intersect_visitor;
pub mod query;
mod traits;
mod visitor;

pub use binary_node::*;
pub use bvh::*;
pub use index::*;
pub use intersect_visitor::*;
pub use traits::*;
pub use visitor::*;

use self::query::TreeQuery;

pub type Nodes<N> = SlotMap<NodeIndex, N>;

pub struct OnDrop {
    object: Object,
    tx: Sender<Object>,
}

impl Drop for OnDrop {
    fn drop(&mut self) {
        if !self.tx.is_disconnected() {
            self.tx.send(self.object).unwrap()
        }
    }
}

new_key_type! {
    pub struct ObjectIndex;
}

/// Marker for where the object is in the tree
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Object {
    pub entity: Entity,
    pub index: ObjectIndex,
}

pub struct CollisionTree<N> {
    nodes: SlotMap<NodeIndex, N>,
    /// Objects removed from the tree due to splits. Bound to be replaced.
    /// Double buffer as insertions may cause new pops.
    root: NodeIndex,

    objects: SlotMap<ObjectIndex, ObjectData>,
    /// Objects that need to be reinserted into the tree
    popped: Vec<Object>,
    tx: Sender<Object>,
    rx: Receiver<Object>,
}

impl<N: CollisionTreeNode> CollisionTree<N> {
    pub fn new(root: N) -> Self {
        let mut nodes = SlotMap::with_key();

        let root = nodes.insert(root);

        let (tx, rx) = flume::unbounded();

        Self {
            nodes,
            root,
            objects: SlotMap::with_key(),
            popped: Vec::new(),
            tx,
            rx,
        }
    }

    /// Get a reference to the collision tree's nodes.
    pub fn nodes(&self) -> &SlotMap<NodeIndex, N> {
        &self.nodes
    }

    pub fn node(&self, index: NodeIndex) -> Option<&N> {
        self.nodes.get(index)
    }

    /// Get a mutable reference to the collision tree's nodes.
    pub fn nodes_mut(&mut self) -> &mut SlotMap<NodeIndex, N> {
        &mut self.nodes
    }

    fn insert(&mut self, entity: Entity, object: ObjectData) -> Object {
        let index = self.objects.insert(object);
        let object = Object { entity, index };

        N::insert(self.root, &mut self.nodes, object, &self.objects);

        object
    }

    pub fn register(&mut self, world: SubWorld<ObjectQuery>, cmd: &mut CommandBuffer) {
        for (e, q) in world.native_query().without::<Object>().iter() {
            let obj: ObjectData = q.into();
            let object = self.insert(e, obj);

            let on_drop = OnDrop {
                object,
                tx: self.tx.clone(),
            };

            cmd.insert(e, (object, on_drop))
        }
    }

    fn handle_removed(&mut self) {
        for object in self.rx.try_iter() {
            let index = N::locate(
                self.root,
                &mut self.nodes,
                object,
                &self.objects.remove(object.index).unwrap(),
            )
            .expect("Object in tree");

            self.nodes[index].remove(object);
        }
    }

    pub fn update(
        &mut self,
        world: SubWorld<(&Object, ObjectQuery)>,
    ) -> Result<(), hecs_schedule::Error> {
        // Update object data
        for (_, (obj, q)) in world.native_query().without::<Static>().iter() {
            let data: ObjectData = q.into();
            self.objects[obj.index] = data;
        }

        self.handle_removed();

        // Update tree
        N::update(self.root, &mut self.nodes, &self.objects, &mut self.popped);

        for object in self.popped.drain(..) {
            N::insert(self.root, &mut self.nodes, object, &self.objects)
        }

        Ok(())
    }

    #[inline]
    pub fn check_collisions<'a, G: Array<Item = &'a ObjectData>>(
        &'a self,
        world: SubWorld<&Collider>,
        events: &mut Events,
    ) -> hecs_schedule::error::Result<()> {
        let colliders = world.try_get_column()?;

        N::check_collisions(&colliders, events, self.root, &self.nodes, &self.objects);

        Ok(())
    }

    /// Queries the tree with a given visitor. Traverses only the nodes that the
    /// visitor accepts and returns an iterator for each node containing the
    /// output of the visited node. Oftentimes, the output of the visitor is an
    /// iterator, which means that a nested iterator can be returned.
    pub fn query<V>(&self, visitor: V) -> TreeQuery<N, V> {
        TreeQuery::new(visitor, self, self.root)
    }

    /// Get a reference to the collision tree's objects.
    pub fn objects(&self) -> &SlotMap<ObjectIndex, ObjectData> {
        &self.objects
    }
}

impl<N: CollisionTreeNode> std::fmt::Debug for CollisionTree<N> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CollisionTree")
            .field("root", &DebugNode::new(self.root, &self.nodes))
            .finish()
    }
}

impl<N: CollisionTreeNode> CollisionTree<N> {
    pub fn register_system(
        world: SubWorld<ObjectQuery>,
        mut cmd: Write<CommandBuffer>,
        mut tree: DefaultResourceMut<Self>,
    ) -> hecs_schedule::error::Result<()> {
        tree.register(world, &mut cmd);

        Ok(())
    }

    pub fn update_system(
        world: SubWorld<(&Object, ObjectQuery)>,
        mut tree: DefaultResourceMut<Self>,
    ) -> hecs_schedule::error::Result<()> {
        tree.update(world)
    }

    pub fn check_collisions_system(
        world: SubWorld<&Collider>,
        tree: DefaultResourceMut<Self>,
        mut events: Write<Events>,
    ) -> hecs_schedule::error::Result<()>
    where
        N: CollisionTreeNode,
    {
        tree.check_collisions::<[&ObjectData; 128]>(world, &mut events)?;

        Ok(())
    }
}

impl<N: CollisionTreeNode + DrawGizmos> CollisionTree<N> {
    pub fn draw_system(tree: DefaultResource<Self>, gizmos: DefaultResourceMut<Gizmos>) {
        tree.draw_gizmos(gizmos, Color::white())
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[record]
pub struct ObjectData {
    pub bounds: BoundingBox,
    pub extended_bounds: BoundingBox,
    pub transform: TransformMatrix,
    pub is_trigger: bool,
    pub is_visible: bool,
    pub is_static: bool,
}

#[derive(Query)]
pub struct ObjectQuery<'a> {
    transform: TransformQuery<'a>,
    collider: &'a Collider,
    is_trigger: Option<&'a Trigger>,
    is_static: Option<&'a Static>,
    is_visible: Option<&'a Visible>,
    velocity: Option<&'a Velocity>,
}

impl<'a> Into<ObjectData> for ObjectQuery<'a> {
    fn into(self) -> ObjectData {
        let transform = self.transform.into_matrix();
        let bounds = self.collider.bounding_box(transform);
        let extended_bounds = if let Some(vel) = self.velocity {
            bounds.expand(**vel * 0.1)
        } else {
            bounds
        };
        ObjectData {
            bounds,
            extended_bounds,
            transform,
            is_trigger: self.is_trigger.is_some(),
            is_visible: self.is_visible.map(|val| val.is_visible()).unwrap_or(true),
            is_static: self.is_static.is_some(),
        }
    }
}

/// Entity with additional contextual data
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EntityPayload {
    pub entity: Entity,
    pub is_trigger: bool,
    pub is_static: bool,
}

impl std::ops::Deref for EntityPayload {
    type Target = Entity;

    fn deref(&self) -> &Self::Target {
        &self.entity
    }
}

impl<N: CollisionTreeNode + DrawGizmos> DrawGizmos for CollisionTree<N> {
    fn draw_gizmos<T: std::ops::DerefMut<Target = Gizmos>>(
        &self,
        mut gizmos: T,
        color: ivy_base::Color,
    ) {
        gizmos.begin_section(any::type_name::<Self>());
        self.root
            .draw_gizmos_recursive(&self.nodes, &mut gizmos, color)
    }
}

use arrayvec::{Array, ArrayVec};
use hecs::{Entity, World};
use ivy_core::{Color, Gizmo, Gizmos, Position, Scale};
use slotmap::{new_key_type, SlotMap};
use ultraviolet::Vec3;

use crate::{Collider, CollisionPrimitive, Sphere};

/// Marker for where the object is in the tree
struct TreeMarker {
    _index: NodeIndex,
}

new_key_type!(
    pub struct NodeIndex;
);

type Nodes<C> = SlotMap<NodeIndex, Node<C>>;

pub struct CollisionTree<C: Array<Item = Object>> {
    nodes: SlotMap<NodeIndex, Node<C>>,
    root: NodeIndex,
}

impl<C: Array<Item = Object>> CollisionTree<C> {
    pub fn new(origin: Vec3, half_extents: Vec3) -> Self {
        let mut nodes = SlotMap::with_key();
        let root = nodes.insert(Node::new(origin, half_extents));
        Self { nodes, root }
    }

    pub fn contains(&self, object: &Object) -> bool {
        self.nodes[self.root].contains(object)
    }

    /// Get a reference to the collision tree's nodes.
    pub fn nodes(&self) -> &SlotMap<NodeIndex, Node<C>> {
        &self.nodes
    }

    /// Get a mutable reference to the collision tree's nodes.
    pub fn nodes_mut(&mut self) -> &mut SlotMap<NodeIndex, Node<C>> {
        &mut self.nodes
    }

    pub fn register(&mut self, world: &mut World) {
        let insterted = world
            .query::<(&Collider, &Position, &Scale)>()
            .without::<TreeMarker>()
            .iter()
            .map(|(e, (collider, position, scale))| {
                Object::new(
                    e,
                    Sphere::new(collider.max_radius() * scale.component_max()),
                    **position,
                )
            })
            .collect::<Vec<_>>();

        insterted.into_iter().for_each(|object| {
            let entity = object.entity;
            let index = self.insert(object);
            world
                .insert_one(entity, TreeMarker { _index: index })
                .unwrap();
        })
    }

    /// Inserts an object into the tree and returns where it was placed. May
    /// split the tree
    pub fn insert(&mut self, object: Object) -> NodeIndex {
        Node::insert(self.root, &mut self.nodes, object)
    }

    pub fn draw_gizmos(&self, gizmos: &mut Gizmos) {
        Node::draw_gizmos(self.root, &self.nodes, 1, gizmos);
    }
}

pub struct Node<C: Array<Item = Object>> {
    objects: ArrayVec<C>,
    origin: Vec3,
    half_extents: Vec3,
    children: Option<[NodeIndex; 2]>,
}

impl<C: Array<Item = Object>> Node<C> {
    pub fn new(origin: Vec3, half_extents: Vec3) -> Self {
        Self {
            objects: ArrayVec::default(),
            origin,
            half_extents,
            children: None,
        }
    }

    /// Inserts into node. Does not check if it is fully contained or if already
    /// in node.
    pub fn insert(current: NodeIndex, nodes: &mut Nodes<C>, object: Object) -> NodeIndex {
        // Check if any child can contain it
        let node = &nodes[current];
        if let Some(child) = node.children.iter().flatten().find(|child| {
            if nodes[**child].contains(&object) {
                eprintln!("Object was contained");
                true
            } else {
                eprintln!("Not contained");
                false
            }
        }) {
            eprintln!("Inserting into child");
            Self::insert(*child, nodes, object)
        } else if node.objects.len() < C::CAPACITY {
            nodes[current].objects.push(object);
            current
        } else {
            // let node = &mut nodes[current];
            Self::split(current, nodes);
            Self::insert(current, nodes, object)
        }
    }

    /// Splits the node in half
    pub fn split(current: NodeIndex, nodes: &mut Nodes<C>) {
        // eprintln!("Splitting");
        let mut center = Vec3::zero();
        let mut max = Vec3::zero();
        let mut min = Vec3::zero();

        let node = &mut nodes[current];
        eprintln!("children: {:?}", node.children);
        assert!(node.children.is_none());

        node.objects.iter().for_each(|val| {
            center += val.origin;
            max = max.max_by_component(val.origin);
            min = min.min_by_component(val.origin);
        });

        let len = node.objects.len();
        let center = center * (1.0 / len as f32);

        let width = (max - min).abs();

        let max = max_axis(width);

        let off = node.half_extents * max * 0.5;

        let extents = node.half_extents - off;
        let a_origin = node.origin - off;
        let b_origin = node.origin + off;

        let rel_center = (center - node.origin) * max;

        let mut a = Node::new(a_origin + rel_center, extents + rel_center);
        let mut b = Node::new(b_origin + rel_center, extents - rel_center);

        // Repartition nodes
        let old = std::mem::replace(&mut node.objects, ArrayVec::new());

        for obj in old {
            if a.contains(&obj) {
                eprintln!("Contained in left");
                a.objects.push(obj)
            } else if b.contains(&obj) {
                eprintln!("Contained in right");
                b.objects.push(obj)
            } else {
                eprintln!("Neither");
                node.objects.push(obj)
            }
        }

        let a = nodes.insert(a);
        let b = nodes.insert(b);

        nodes[current].children = Some([a, b]);
    }

    /// Returns true if the object bounded by a sphere fits in the node.
    pub fn contains(&self, object: &Object) -> bool {
        object.origin.x + object.bound.radius < self.origin.x + self.half_extents.x
            && object.origin.x - object.bound.radius > self.origin.x - self.half_extents.x
            && object.origin.y + object.bound.radius < self.origin.y + self.half_extents.y
            && object.origin.y - object.bound.radius > self.origin.y - self.half_extents.y
            && object.origin.z + object.bound.radius < self.origin.z + self.half_extents.z
            && object.origin.z - object.bound.radius > self.origin.z - self.half_extents.z
    }

    pub fn draw_gizmos(current: NodeIndex, nodes: &Nodes<C>, depth: usize, gizmos: &mut Gizmos) {
        let node = &nodes[current];

        gizmos.push(Gizmo::Sphere {
            origin: node.origin,
            color: Color::red(),
            radius: 0.2,
            corner_radius: 1.0,
        });

        gizmos.push(Gizmo::Cube {
            origin: node.origin,
            color: Color::red(),
            half_extents: node.half_extents,
            radius: 0.1 / depth as f32,
            corner_radius: 1.0,
        });

        node.children
            .iter()
            .flatten()
            .for_each(|val| Self::draw_gizmos(*val, nodes, depth + 1, gizmos))
    }
}

/// Represents an entity with extra collider information
pub struct Object {
    entity: Entity,
    bound: Sphere,
    origin: Vec3,
}

impl Object {
    pub fn new(entity: Entity, bound: Sphere, origin: Vec3) -> Self {
        Self {
            entity,
            bound,
            origin,
        }
    }

    /// Get a reference to the object's entity.
    pub fn entity(&self) -> Entity {
        self.entity
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

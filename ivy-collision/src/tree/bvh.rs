use flax::component::ComponentValue;
use glam::Vec3;
use ivy_core::{Cube, DrawGizmos, Events, GizmosSection};
use ordered_float::OrderedFloat;
use palette::{
    named::{BLUE, GREEN, YELLOW},
    Srgb, WithAlpha,
};
use slotmap::SlotMap;

use crate::{
    intersect, BoundingBox, Collision, CollisionTreeNode, CollisionTreeObject, NodeIndex,
    NodeState, Nodes, ObjectData, ObjectIndex,
};

const MARGIN: f32 = 1.2;
const NODE_CAPACITY: usize = 16;

#[derive(Debug, Clone)]
pub struct BvhNode {
    bounds: BoundingBox,
    objects: Vec<ObjectIndex>,
    children: Option<[NodeIndex; 2]>,
    depth: u32,
    state: NodeState,
}

impl Default for BvhNode {
    fn default() -> Self {
        Self {
            bounds: Default::default(),
            objects: Default::default(),
            children: Default::default(),
            depth: Default::default(),
            state: NodeState::Static,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    X,
    Y,
    Z,
}

impl Default for Axis {
    fn default() -> Self {
        Self::X
    }
}

impl From<Axis> for usize {
    fn from(value: Axis) -> Self {
        match value {
            Axis::X => 0,
            Axis::Y => 1,
            Axis::Z => 2,
        }
    }
}

impl Axis {
    fn rotate(&self) -> Self {
        match self {
            Axis::X => Axis::Y,
            Axis::Y => Axis::Z,
            Axis::Z => Axis::X,
        }
    }
}

impl BvhNode {
    pub fn new(bounds: BoundingBox) -> Self {
        Self {
            bounds,
            objects: Vec::new(),
            children: None,
            depth: 0,
            state: NodeState::Static,
        }
    }

    fn from_objects(
        nodes: &mut Nodes<Self>,
        objects: &[ObjectIndex],
        data: &SlotMap<ObjectIndex, ObjectData>,
        depth: u32,
    ) -> NodeIndex {
        let state = objects
            .iter()
            .fold(NodeState::Static, |s, &v| s.merge(data[v].state));

        let bounds = Self::calculate_bounds(&objects, data, state);

        let node = Self {
            bounds,
            objects: objects.to_vec(),
            children: None,
            depth,
            state,
        };

        let index = nodes.insert(node);

        // Recurse if the number of objects are more than allowed
        Self::try_split(index, nodes, data);

        index
    }

    /// Splits the node into subtrees as far as is needed
    fn try_split(
        index: NodeIndex,
        nodes: &mut Nodes<Self>,
        data: &SlotMap<ObjectIndex, ObjectData>,
    ) {
        let node = &mut nodes[index];
        if node.objects.len() <= NODE_CAPACITY {
            return;
        }

        let bounds = Self::calculate_bounds(&node.objects, data, NodeState::Static).size();
        let axis = if bounds.x > bounds.y {
            if bounds.x > bounds.z {
                Axis::X
            } else {
                Axis::Z
            }
        } else if bounds.y > bounds.x {
            Axis::Y
        } else {
            Axis::Z
        };

        // Sort by axis and select the median
        node.sort_by_axis(axis, data);
        let median = node.objects.len() / 2;

        let left = &node.objects[0..median].to_vec();
        let right = &node.objects[median..].to_vec();
        assert_eq!(left.len() + right.len(), node.objects.len());
        let depth = node.depth + 1;

        node.objects.clear();

        let left = Self::from_objects(nodes, left, data, depth);
        let right = Self::from_objects(nodes, right, data, depth);

        nodes[index].children = Some([left, right]);
    }

    pub fn calculate_bounds_incremental(&self, object: &ObjectData) -> BoundingBox {
        self.bounds
            .merge(object.extended_bounds.rel_margin(if !object.is_movable() {
                1.0
            } else {
                MARGIN
            }))
    }

    /// Updates the bounds of the object
    pub fn calculate_bounds(
        objects: &[ObjectIndex],
        data: &SlotMap<ObjectIndex, ObjectData>,
        state: NodeState,
    ) -> BoundingBox {
        let mut l = Vec3::new(f32::MAX, f32::MAX, f32::MAX);
        let mut r = Vec3::new(f32::MIN, f32::MIN, f32::MIN);

        for &val in objects.iter() {
            let bounds = data[val].extended_bounds;
            l = l.min(bounds.min);
            r = r.max(bounds.max);
        }

        BoundingBox::from_corners(l, r).margin(if state.is_dynamic() { MARGIN } else { 1.0 })
    }

    fn sort_by_axis(&mut self, axis: Axis, data: &SlotMap<ObjectIndex, ObjectData>) {
        let axis = axis.into();

        self.objects
            .sort_unstable_by_key(|val| OrderedFloat(data[*val].bounds.midpoint()[axis]))
    }

    fn traverse<F: FnMut(&Self, &Self)>(
        a: NodeIndex,
        b: NodeIndex,
        nodes: &Nodes<Self>,
        on_overlap: &mut F,
    ) {
        let a_node = &nodes[a];
        let b_node = &nodes[b];

        // Both leaves are dormant
        if a_node.state.dormant() && b_node.state.dormant() {
            return;
        }

        if !a_node.bounds.overlaps(b_node.bounds) {
            return;
        }

        match (a_node.children, b_node.children) {
            // Both are leaves and intersecting
            (None, None) => on_overlap(a_node, b_node),
            // Traverse the other tree
            (None, Some([l, r])) => {
                Self::traverse(a, l, nodes, on_overlap);
                Self::traverse(a, r, nodes, on_overlap);
            }
            // Traverse the current tree
            (Some([l, r]), None) => {
                Self::traverse(l, b, nodes, on_overlap);
                Self::traverse(r, b, nodes, on_overlap);
            }
            (Some([l0, r0]), Some([l1, r1])) => {
                Self::traverse(l0, l1, nodes, on_overlap);
                Self::traverse(l0, r1, nodes, on_overlap);
                Self::traverse(r0, l1, nodes, on_overlap);
                Self::traverse(r0, r1, nodes, on_overlap);
            }
        }
    }

    /// Collapses a whole tree and fills `objects` with the objects in the tree
    fn collapse(index: NodeIndex, nodes: &mut Nodes<Self>, objects: &mut Vec<ObjectIndex>) {
        let node = &mut nodes[index];

        objects.append(&mut node.objects);

        if let Some([l, r]) = node.children.take() {
            Self::collapse(l, nodes, objects);
            Self::collapse(r, nodes, objects);
            nodes.remove(l).unwrap();
            nodes.remove(r).unwrap();
        }
    }

    fn update_impl(
        index: NodeIndex,
        nodes: &mut Nodes<Self>,
        data: &SlotMap<ObjectIndex, ObjectData>,
        to_refit: &mut Vec<ObjectIndex>,
        changed: &mut usize,
    ) -> NodeState {
        let node = &mut nodes[index];

        if node.state.is_static() && *changed == 0 {
            return node.state;
        }

        // Traverse children
        if let Some([left, right]) = node.children {
            assert!(node.objects.is_empty());
            let l = Self::update_impl(left, nodes, data, to_refit, changed);
            let r = Self::update_impl(right, nodes, data, to_refit, changed);
            let new_state = l.merge(r);

            nodes[index].state = new_state;
            new_state
        } else {
            let bounds = node.bounds;

            let mut removed = 0;
            let mut new_state = NodeState::Static;

            node.objects.retain(|&val| {
                let obj = match data.get(val) {
                    Some(val) => val,
                    // Entity was removed
                    None => {
                        removed += 1;
                        *changed -= 1;
                        return false;
                    }
                };

                new_state = new_state.merge(obj.state);

                if bounds.contains(obj.bounds) {
                    true
                } else {
                    removed += 1;
                    to_refit.push(val);
                    false
                }
            });

            if removed > 0 {
                node.bounds = Self::calculate_bounds(&node.objects, data, new_state);
            }

            node.state = new_state;
            new_state
        }
    }
}

impl CollisionTreeNode for BvhNode {
    fn objects(&self) -> &[ObjectIndex] {
        &self.objects
    }

    fn insert(
        index: NodeIndex,
        nodes: &mut Nodes<Self>,
        object: ObjectIndex,
        data: &SlotMap<ObjectIndex, ObjectData>,
    ) {
        let node = &mut nodes[index];
        let obj = &data[object];

        // Make bound fit object
        node.bounds = node.calculate_bounds_incremental(obj);

        node.state = node.state.merge(obj.state);

        // Internal node
        if let Some([left, right]) = node.children {
            assert!(node.objects.is_empty());
            if nodes[left].bounds.contains(obj.bounds) {
                Self::insert(left, nodes, object, data)
            } else if nodes[right].bounds.contains(obj.bounds) {
                Self::insert(right, nodes, object, data)
            }
            // Object did not fit in any child.
            // Gather up both children and all descendants, and re-add all objects by splitting.
            else {
                let mut objects = vec![object];
                Self::collapse(index, nodes, &mut objects);

                let node = &mut nodes[index];

                node.bounds = Self::calculate_bounds(&objects, data, node.state);
                node.objects = objects;
                Self::try_split(index, nodes, data);
            }
        } else {
            node.objects.push(object);

            // Split
            Self::try_split(index, nodes, data);
        }
    }

    fn remove(&mut self, object: ObjectIndex) -> Option<ObjectIndex> {
        if let Some(idx) = self.objects.iter().position(|&val| val == object) {
            Some(self.objects.swap_remove(idx))
        } else {
            None
        }
    }

    fn bounds(&self) -> BoundingBox {
        self.bounds
    }

    fn children(&self) -> &[NodeIndex] {
        match &self.children {
            Some(val) => val,
            None => &[],
        }
    }

    fn update(
        index: NodeIndex,
        nodes: &mut Nodes<Self>,
        data: &SlotMap<ObjectIndex, ObjectData>,
        to_refit: &mut Vec<ObjectIndex>,
        despawned: &mut usize,
    ) {
        unimplemented!()
        // Self::update_impl(index, nodes, data, to_refit, despawned);
    }

    fn check_collisions(
        events: &Events,
        index: NodeIndex,
        nodes: &Nodes<Self>,
        data: &SlotMap<ObjectIndex, ObjectData>,
    ) {
        let mut on_overlap = |a: &Self, b: &Self| {
            assert!(a.is_leaf());
            assert!(b.is_leaf());
            for &a in a.objects() {
                let a_obj = &data[a];
                for &b in b.objects() {
                    let b_obj = &data[b];
                    // if let Some(collision) = check_collision(data, *a, a_obj, *b, b_obj) {
                    //     events.send(collision)
                    // }
                }
            }
        };

        // check if children overlap
        let node = &nodes[index];
        if let Some([left, right]) = node.children {
            assert!(node.objects.is_empty());
            Self::traverse(left, right, nodes, &mut on_overlap);
            Self::check_collisions(events, left, nodes, data);
            Self::check_collisions(events, right, nodes, data);
        } else if !node.state.dormant() {
            // Check collisions for objects in the same leaf
            for (i, &a) in node.objects.iter().enumerate() {
                let a_obj = &data[a];

                for &b in node.objects.iter().skip(i + 1) {
                    assert_ne!(a, b);
                    let b_obj = &data[b];
                    // if let Some(collision) = check_collision(data, *a, a_obj, *b, b_obj) {
                    //     events.send(collision)
                    // }
                }
            }
        }
    }
}

impl DrawGizmos for BvhNode {
    fn draw_primitives(&self, gizmos: &mut GizmosSection) {
        if !self.is_leaf() {
            return;
        }

        let color = match self.state {
            NodeState::Dynamic => GREEN,
            NodeState::Static => BLUE,
            NodeState::Sleeping => YELLOW,
        };

        gizmos.draw(Cube {
            origin: self.bounds.midpoint(),
            half_extents: self.bounds.size() / 2.0,
            color: Srgb::from_format(color).with_alpha(1.0),
            ..Default::default()
        })
    }
}

fn check_collision(
    data: &SlotMap<ObjectIndex, ObjectData>,
    a: CollisionTreeObject,
    a_obj: &ObjectData,
    b: CollisionTreeObject,
    b_obj: &ObjectData,
) -> Option<Collision> {
    if !a_obj.bounds.overlaps(b_obj.bounds) {
        return None;
    }

    let a_data = &data[a.index];
    let b_data = &data[b.index];

    if let Some(contact) = intersect(
        &a_obj.transform,
        &b_obj.transform,
        &a_data.collider,
        &b_data.collider,
    ) {
        let collision = Collision {
            a: crate::EntityPayload {
                entity: a.entity,
                is_trigger: a_obj.is_trigger,
                state: a_obj.state,
            },
            b: crate::EntityPayload {
                entity: b.entity,
                is_trigger: b_obj.is_trigger,
                state: b_obj.state,
            },
            contact,
        };

        Some(collision)
    } else {
        None
    }
}

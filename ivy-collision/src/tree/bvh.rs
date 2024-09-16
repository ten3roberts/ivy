use std::collections::HashSet;

use glam::Vec3;
use ivy_core::{
    gizmos::{Cube, GizmosSection, Sphere, DEFAULT_THICKNESS},
    Color, ColorExt,
};
use ordered_float::OrderedFloat;
use slotmap::SlotMap;

use crate::{Body, BodyIndex, BoundingBox, CollisionTreeNode, NodeIndex, NodeState, Nodes};

pub(crate) const MARGIN: f32 = 1.2;
const NODE_CAPACITY: usize = 1;

#[derive(Debug, Clone)]
pub struct BvhNode {
    current_bounds: BoundingBox,
    pub(crate) allocated_bounds: BoundingBox,
    objects: Vec<BodyIndex>,
    children: Option<[NodeIndex; 2]>,
    state: NodeState,
    dirty_bounds: bool,
}

impl Default for BvhNode {
    fn default() -> Self {
        Self {
            current_bounds: Default::default(),
            allocated_bounds: Default::default(),
            objects: Default::default(),
            children: Default::default(),
            state: NodeState::Static,
            dirty_bounds: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Axis {
    X,
    Y,
    Z,
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

impl BvhNode {
    pub fn new(bounds: BoundingBox) -> Self {
        Self {
            current_bounds: bounds,
            allocated_bounds: bounds,
            objects: Vec::new(),
            children: None,
            state: NodeState::Static,
            dirty_bounds: false,
        }
    }

    fn from_objects(
        nodes: &mut Nodes<Self>,
        objects: Vec<BodyIndex>,
        data: &mut SlotMap<BodyIndex, Body>,
    ) -> (NodeIndex, u32) {
        let state = objects
            .iter()
            .fold(NodeState::Static, |s, &v| s.merge(data[v].state));

        let bounds = Self::calculate_bounds(&objects, data);

        let outer_bounds = bounds.rel_margin(state.inflate_amount());
        let node = Self {
            current_bounds: bounds,
            objects,
            children: None,
            state,
            dirty_bounds: false,
            allocated_bounds: outer_bounds,
        };

        let index = nodes.insert(node);

        // Recurse if the number of objects are more than allowed
        let height = Self::try_split(index, nodes, data);
        let node = &mut nodes[index];
        for &object in &node.objects {
            let data = &mut data[object];
            data.node = index;
        }

        (index, height + 1)
    }

    /// Splits the node into subtrees as far as is needed
    fn try_split(
        index: NodeIndex,
        nodes: &mut Nodes<Self>,
        data: &mut SlotMap<BodyIndex, Body>,
    ) -> u32 {
        let node = &mut nodes[index];
        if node.objects.len() <= NODE_CAPACITY {
            return 0;
        }

        let bounds = Self::calculate_bounds(&node.objects, data).size();
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

        let left = node.objects[0..median].to_vec();
        let right = node.objects[median..].to_vec();
        assert_eq!(left.len() + right.len(), node.objects.len());

        node.objects.clear();

        let (left, left_h) = Self::from_objects(nodes, left, data);
        let (right, right_h) = Self::from_objects(nodes, right, data);

        let new_height = left_h.max(right_h) + 1;
        let node = &mut nodes[index];

        node.children = Some([left, right]);
        new_height
    }

    pub fn calculate_bounds_incremental(&self, object: &Body) -> BoundingBox {
        self.current_bounds
            .merge(object.extended_bounds.rel_margin(if !object.is_movable() {
                1.0
            } else {
                MARGIN
            }))
    }

    /// Updates the bounds of the object
    pub fn calculate_bounds(objects: &[BodyIndex], data: &SlotMap<BodyIndex, Body>) -> BoundingBox {
        let mut l = Vec3::new(f32::MAX, f32::MAX, f32::MAX);
        let mut r = Vec3::new(f32::MIN, f32::MIN, f32::MIN);

        for &val in objects.iter() {
            let bounds = data[val].extended_bounds;
            l = l.min(bounds.min);
            r = r.max(bounds.max);
        }

        BoundingBox::from_corners(l, r)
    }

    fn sort_by_axis(&mut self, axis: Axis, data: &SlotMap<BodyIndex, Body>) {
        let axis = axis.into();

        self.objects
            .sort_unstable_by_key(|val| OrderedFloat(data[*val].bounds.midpoint()[axis]))
    }

    pub fn traverse_overlapping_nodes(
        a: NodeIndex,
        b: NodeIndex,
        nodes: &Nodes<Self>,
        on_overlap: &mut impl FnMut(NodeIndex, &Self, NodeIndex, &Self),
    ) {
        let a_node = &nodes[a];
        let b_node = &nodes[b];

        // Both leaves are dormant
        if a_node.state.dormant() && b_node.state.dormant() {
            return;
        }

        if !a_node.current_bounds.overlaps(b_node.current_bounds) {
            return;
        }

        match (a_node.children, b_node.children) {
            // Both are leaves and intersecting
            (None, None) => on_overlap(a, a_node, b, b_node),
            // Traverse the other tree
            (None, Some([l, r])) => {
                Self::traverse_overlapping_nodes(a, l, nodes, on_overlap);
                Self::traverse_overlapping_nodes(a, r, nodes, on_overlap);
            }
            // Traverse the current tree
            (Some([l, r]), None) => {
                Self::traverse_overlapping_nodes(l, b, nodes, on_overlap);
                Self::traverse_overlapping_nodes(r, b, nodes, on_overlap);
            }
            (Some([l0, r0]), Some([l1, r1])) => {
                Self::traverse_overlapping_nodes(l0, l1, nodes, on_overlap);
                Self::traverse_overlapping_nodes(l0, r1, nodes, on_overlap);
                Self::traverse_overlapping_nodes(r0, l1, nodes, on_overlap);
                Self::traverse_overlapping_nodes(r0, r1, nodes, on_overlap);
            }
        }
    }

    pub fn check_collisions<'a>(
        index: NodeIndex,
        nodes: &Nodes<Self>,
        data: &'a SlotMap<BodyIndex, Body>,
        on_collision: &mut impl FnMut(BodyIndex, &'a Body, BodyIndex, &'a Body),
    ) {
        let mut on_overlap = |_, a: &Self, _, b: &Self| {
            assert!(a.is_leaf());
            assert!(b.is_leaf());

            for &a in a.objects() {
                let a_obj = &data[a];
                for &b in b.objects() {
                    let b_obj = &data[b];

                    if a_obj.bounds.overlaps(b_obj.bounds) {
                        on_collision(a, a_obj, b, b_obj);
                    }
                }
            }
        };

        let node = &nodes[index];

        if let Some([left, right]) = node.children {
            assert!(node.objects.is_empty());
            Self::traverse_overlapping_nodes(left, right, nodes, &mut on_overlap);

            Self::check_collisions(left, nodes, data, on_collision);
            Self::check_collisions(right, nodes, data, on_collision);
        } else if !node.state.dormant() {
            // Check collisions for objects in the same leaf
            for (i, &a) in node.objects.iter().enumerate() {
                let a_obj = &data[a];

                for &b in node.objects.iter().skip(i + 1) {
                    assert_ne!(a, b);
                    let b_obj = &data[b];
                    if a_obj.bounds.overlaps(b_obj.bounds) {
                        on_collision(a, a_obj, b, b_obj);
                    }
                }
            }
        }
    }

    /// Merge a whole tree and fills `objects` with the objects in the tree
    fn merge(index: NodeIndex, nodes: &mut Nodes<Self>, objects: &mut Vec<BodyIndex>) {
        let node = &mut nodes[index];

        node.dirty_bounds = true;
        objects.append(&mut node.objects);
        node.objects.clear();

        if let Some([l, r]) = node.children.take() {
            Self::merge(l, nodes, objects);
            Self::merge(r, nodes, objects);
            nodes.remove(l).unwrap();
            nodes.remove(r).unwrap();
        }
    }

    pub fn insert(
        index: NodeIndex,
        nodes: &mut Nodes<Self>,
        object: BodyIndex,
        data: &mut SlotMap<BodyIndex, Body>,
    ) {
        let node = &mut nodes[index];
        let obj = &data[object];

        assert!(node.allocated_bounds.contains(obj.extended_bounds));
        // Refit bounds
        // NOTE: parents are refit at the end of the frame
        // node.allocated_bounds = node.current_bounds.margin(node.state.inflate_amount());

        node.state = node.state.merge(obj.state);

        // Internal node
        if let Some([left, right]) = node.children {
            assert!(node.objects.is_empty());
            if nodes[left].allocated_bounds.contains(obj.extended_bounds) {
                Self::insert(left, nodes, object, data)
            } else if nodes[right].allocated_bounds.contains(obj.extended_bounds) {
                Self::insert(right, nodes, object, data)
            } else {
                // Object did not fit in any child. Gather up both children and all descendants, and re-add all objects by splitting.
                let mut objects = vec![];
                Self::merge(index, nodes, &mut objects);
                objects.push(object);

                let node = &mut nodes[index];

                node.current_bounds = Self::calculate_bounds(&objects, data);
                let old_allocation = node.allocated_bounds;
                node.allocated_bounds = node.current_bounds.rel_margin(node.state.inflate_amount());
                assert!(old_allocation.contains(node.allocated_bounds));
                node.objects = objects;
                Self::try_split(index, nodes, data);

                let node = &mut nodes[index];

                for &object in &node.objects {
                    let data = &mut data[object];
                    data.node = index;
                }
            }
        } else {
            node.objects.push(object);
            {
                let data = &mut data[object];
                data.node = index;
            }

            // Split
            Self::try_split(index, nodes, data);
        }
    }

    pub fn rebalance(
        index: NodeIndex,
        nodes: &mut Nodes<Self>,
        data: &mut SlotMap<BodyIndex, Body>,
    ) -> u32 {
        if let Some([l, r]) = nodes[index].children {
            let l = Self::rebalance(l, nodes, data);
            let r = Self::rebalance(r, nodes, data);

            if (l as i32 - r as i32).abs() > 1 {
                let mut objects = vec![];
                Self::merge(index, nodes, &mut objects);

                let node = &mut nodes[index];

                let bounds = Self::calculate_bounds(&objects, data);
                let outer_bounds = bounds.rel_margin(node.state.inflate_amount());

                let node = &mut nodes[index];
                node.current_bounds = bounds;
                node.allocated_bounds = outer_bounds;
                node.objects = objects;

                Self::try_split(index, nodes, data) + 1
            } else {
                l.max(r) + 1
            }
        } else {
            0
        }
    }

    pub fn update_bounds(
        index: NodeIndex,
        nodes: &mut Nodes<Self>,
        objects: &SlotMap<BodyIndex, Body>,
    ) -> BoundingBox {
        if let Some([l, r]) = nodes[index].children {
            let l_allocated = nodes[l].allocated_bounds;
            let r_allocated = nodes[r].allocated_bounds;
            let node = &nodes[index];
            assert!(node.allocated_bounds.contains(l_allocated));
            assert!(node.allocated_bounds.contains(r_allocated));

            let l = Self::update_bounds(l, nodes, objects);
            let r = Self::update_bounds(r, nodes, objects);
            let bounds = l.merge(r);
            nodes[index].current_bounds = bounds;
            assert!(
                nodes[index].allocated_bounds.contains(bounds),
                "{:?} does not contain {:?}",
                nodes[index].allocated_bounds,
                bounds
            );
            bounds
        } else {
            let node = &mut nodes[index];
            node.current_bounds = Self::calculate_bounds(&node.objects, objects);
            assert!(node.allocated_bounds.contains(node.current_bounds));
            node.current_bounds
        }
    }

    pub fn allocated_bounds(&self) -> BoundingBox {
        self.allocated_bounds
    }
}

impl CollisionTreeNode for BvhNode {
    fn objects(&self) -> &[BodyIndex] {
        &self.objects
    }

    fn remove(&mut self, object: BodyIndex) -> Option<BodyIndex> {
        let idx = self.objects.iter().position(|&val| val == object)?;

        self.dirty_bounds = true;
        let object = self.objects.swap_remove(idx);
        Some(object)
    }

    fn bounds(&self) -> BoundingBox {
        self.current_bounds
    }

    fn children(&self) -> &[NodeIndex] {
        match &self.children {
            Some(val) => val,
            None => &[],
        }
    }
}

impl BvhNode {
    pub fn draw_gizmos_recursive(
        index: NodeIndex,
        nodes: &Nodes<BvhNode>,
        gizmos: &mut GizmosSection,
        data: &SlotMap<BodyIndex, Body>,
        overlapping: &mut HashSet<NodeIndex>,
        depth: usize,
    ) {
        if depth > 50 {
            panic!("");
        }
        let node = &nodes[index];

        node.draw_primitives(
            gizmos,
            data,
            Color::from_hsla(
                depth as f32 * 15.0,
                1.0,
                if overlapping.contains(&index) {
                    0.5
                } else {
                    0.1
                },
                if overlapping.contains(&index) {
                    0.5
                } else {
                    0.1
                },
            ),
        );

        if let Some([l, r]) = node.children {
            Self::traverse_overlapping_nodes(l, r, nodes, &mut |a, _, b, _| {
                overlapping.extend([a, b]);
            });

            Self::draw_gizmos_recursive(l, nodes, gizmos, data, overlapping, depth + 1);
            Self::draw_gizmos_recursive(r, nodes, gizmos, data, overlapping, depth + 1);
        }
    }

    pub fn draw_primitives(
        &self,
        gizmos: &mut GizmosSection,
        data: &SlotMap<BodyIndex, Body>,
        color: Color,
    ) {
        // if !self.is_leaf() {
        //     return;
        // }

        // let color = match self.state {
        //     NodeState::Dynamic => GREEN,
        //     NodeState::Static => BLUE,
        //     NodeState::Sleeping => YELLOW,
        // };
        // let color = Srgb::from_format(color).with_alpha(1.0);

        gizmos.draw(Cube {
            min: self.current_bounds.min,
            max: self.current_bounds.max,
            color,
            line_radius: DEFAULT_THICKNESS,
        });

        for &object in self.objects() {
            let data = &data[object];

            gizmos.draw(Sphere {
                origin: data.transform.transform_point3(Vec3::ZERO),
                radius: 0.1,
                color,
            })
        }
    }
}

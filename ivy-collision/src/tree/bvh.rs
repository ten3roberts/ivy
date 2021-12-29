use hecs::{Column, Component};
use ivy_base::{Color, DrawGizmos, Events};
use ordered_float::OrderedFloat;
use slotmap::SlotMap;
use smallvec::{smallvec, Array, SmallVec};
use ultraviolet::Vec3;

use crate::{
    intersect, BoundingBox, Collider, Collision, CollisionTreeNode, NodeIndex, Nodes, Object,
    ObjectData, ObjectIndex,
};

const MARGIN: f32 = 1.2;

#[derive(Debug, Clone)]
pub struct BVHNode<O: Array<Item = Object> = [Object; 1]> {
    bounds: BoundingBox,
    objects: SmallVec<O>,
    axis: Axis,
    children: Option<[NodeIndex; 2]>,
    depth: u32,
    /// all objects inside this subtree is static
    is_static: bool,
}

impl<O: Array<Item = Object>> Default for BVHNode<O> {
    fn default() -> Self {
        Self {
            bounds: Default::default(),
            objects: Default::default(),
            axis: Default::default(),
            children: Default::default(),
            depth: Default::default(),
            is_static: Default::default(),
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

impl Into<usize> for Axis {
    fn into(self) -> usize {
        match self {
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

impl<O> BVHNode<O>
where
    O: Array<Item = Object>,
{
    pub fn new(bounds: BoundingBox, axis: Axis) -> Self {
        Self {
            bounds,
            objects: SmallVec::new(),
            axis,
            children: None,
            depth: 0,
            is_static: true,
        }
    }

    fn from_objects(
        nodes: &mut Nodes<Self>,
        objects: SmallVec<O>,
        data: &SlotMap<ObjectIndex, ObjectData>,
        axis: Axis,
        depth: u32,
    ) -> NodeIndex {
        let is_static = objects.iter().all(|val| data[val.index].is_static);

        let bounds = Self::calculate_bounds(&objects, data, is_static);

        let node = Self {
            bounds,
            objects: objects.into(),
            axis,
            children: None,
            depth,
            is_static,
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
        if node.objects.len() <= node.objects.inline_size() {
            return;
        }

        // Sort by axis and select the median
        node.sort_by_axis(data);
        let median = node.objects.len() / 2;
        let left: SmallVec<O> = node.objects[0..median].into();
        let right: SmallVec<O> = node.objects[median..].into();
        assert_eq!(left.len() + right.len(), node.objects.len());
        let new_axis = node.axis.rotate();
        let depth = node.depth + 1;

        node.objects.clear();

        let left = Self::from_objects(nodes, left, data, new_axis, depth);
        let right = Self::from_objects(nodes, right, data, new_axis, depth);

        nodes[index].children = Some([left, right]);
    }

    pub fn calculate_bounds_incremental(&self, object: &ObjectData) -> BoundingBox {
        self.bounds
            .merge(
                object
                    .extended_bounds
                    .rel_margin(if object.is_static { 1.0 } else { MARGIN }),
            )
    }

    /// Updates the bounds of the object
    pub fn calculate_bounds(
        objects: &[Object],
        data: &SlotMap<ObjectIndex, ObjectData>,
        is_static: bool,
    ) -> BoundingBox {
        let mut l = Vec3::new(f32::MAX, f32::MAX, f32::MAX);
        let mut r = Vec3::new(f32::MIN, f32::MIN, f32::MIN);

        objects.iter().for_each(|val| {
            let bounds = data[val.index].extended_bounds;
            let (l_obj, r_obj) = bounds.into_corners();
            l = l.min_by_component(l_obj);
            r = r.max_by_component(r_obj);
        });

        BoundingBox::from_corners(l, r).margin(if is_static { 1.0 } else { MARGIN })
    }

    fn sort_by_axis(&mut self, data: &SlotMap<ObjectIndex, ObjectData>) {
        let axis = self.axis.into();
        self.objects
            .sort_unstable_by_key(|val| OrderedFloat(data[val.index].bounds.origin[axis]))
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
        if a_node.is_static && b_node.is_static {
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
    fn collapse(index: NodeIndex, nodes: &mut Nodes<Self>, objects: &mut SmallVec<O>) {
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
        to_refit: &mut Vec<Object>,
    ) -> bool {
        let node = &mut nodes[index];

        if node.is_static {
            return true;
        }

        if let Some([left, right]) = node.children {
            assert!(node.objects.is_empty());
            let l = Self::update_impl(left, nodes, data, to_refit);
            let r = Self::update_impl(right, nodes, data, to_refit);
            let is_static = l && r;

            nodes[index].is_static = false;
            is_static
        } else {
            let bounds = node.bounds;

            let mut removed = 0;
            let mut is_static = true;

            node.objects.retain(|val| {
                let obj = data[val.index];

                is_static = is_static && obj.is_static;

                if bounds.contains(obj.bounds) {
                    true
                } else {
                    removed += 1;
                    to_refit.push(*val);
                    false
                }
            });

            if removed > 0 {
                node.bounds = Self::calculate_bounds(&node.objects, data, is_static);
            }

            node.is_static = is_static;
            is_static
        }
    }
}

impl<O: Array<Item = Object> + Component> CollisionTreeNode for BVHNode<O> {
    fn objects(&self) -> &[Object] {
        &self.objects
    }

    fn insert(
        index: NodeIndex,
        nodes: &mut Nodes<Self>,
        object: Object,
        data: &SlotMap<ObjectIndex, ObjectData>,
    ) {
        let node = &mut nodes[index];
        let obj = &data[object.index];

        // Make bound fit object
        node.bounds = node.calculate_bounds_incremental(&obj);

        node.is_static = node.is_static && obj.is_static;

        // Internal node
        if let Some([left, right]) = node.children {
            assert!(node.objects.is_empty());
            if nodes[left].bounds.contains(obj.bounds) {
                return Self::insert(left, nodes, object, data);
            } else if nodes[right].bounds.contains(obj.bounds) {
                return Self::insert(right, nodes, object, data);
            }
            // Object did not fit in any child.
            // Gather up both children and all descendants, and re-add all objects by splitting.
            else {
                let mut objects = smallvec![object];
                Self::collapse(index, nodes, &mut objects);

                let node = &mut nodes[index];

                node.bounds = Self::calculate_bounds(&objects, data, node.is_static);
                node.objects = objects;
                Self::try_split(index, nodes, data);
            }
        } else {
            node.objects.push(object);

            // Split
            Self::try_split(index, nodes, data);
        }
    }

    fn remove(&mut self, object: Object) -> Option<Object> {
        if let Some(idx) = self.objects.iter().position(|val| *val == object) {
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
        to_refit: &mut Vec<Object>,
    ) {
        Self::update_impl(index, nodes, data, to_refit);
    }

    fn check_collisions(
        colliders: &Column<Collider>,
        events: &Events,
        index: NodeIndex,
        nodes: &Nodes<Self>,
        data: &SlotMap<ObjectIndex, ObjectData>,
    ) {
        let mut on_overlap = |a: &Self, b: &Self| {
            assert!(a.is_leaf());
            assert!(b.is_leaf());
            for a in a.objects() {
                let a_obj = data[a.index];
                for b in b.objects() {
                    let b_obj = data[b.index];
                    if let Some(collision) = check_collision(colliders, *a, &a_obj, *b, &b_obj) {
                        events.send(collision)
                    }
                }
            }
        };

        // check if children overlap
        let node = &nodes[index];
        if let Some([left, right]) = node.children {
            assert!(node.objects.is_empty());
            Self::traverse(left, right, nodes, &mut on_overlap);
            Self::check_collisions(colliders, events, left, nodes, data);
            Self::check_collisions(colliders, events, right, nodes, data);
        } else if !node.is_static {
            // Check collisions for objects in the same leaf
            for (i, a) in node.objects.iter().enumerate() {
                let a_obj = data[a.index];

                for b in node.objects.iter().skip(i + 1) {
                    assert_ne!(a, b);
                    let b_obj = data[b.index];
                    if let Some(collision) = check_collision(colliders, *a, &a_obj, *b, &b_obj) {
                        events.send(collision)
                    }
                }
            }
        }
    }
}

impl<O: Array<Item = Object> + Component> DrawGizmos for BVHNode<O> {
    fn draw_gizmos<T: std::ops::DerefMut<Target = ivy_base::Gizmos>>(
        &self,
        mut gizmos: T,
        _: Color,
    ) {
        if !self.is_leaf() {
            return;
        }

        let color = if self.is_static {
            Color::blue()
        } else {
            Color::yellow()
        };

        gizmos.draw(ivy_base::Gizmo::Cube {
            origin: self.bounds.origin,
            color,
            half_extents: self.bounds.extents,
            radius: 0.01,
        })
    }
}

fn check_collision(
    colliders: &Column<Collider>,
    a: Object,
    a_obj: &ObjectData,
    b: Object,
    b_obj: &ObjectData,
) -> Option<Collision> {
    if !a_obj.bounds.overlaps(b_obj.bounds) {
        return None;
    }

    let a_coll = colliders.get(a.entity).expect("Collider");
    let b_coll = colliders.get(b.entity).expect("Collider");

    if let Some(contact) = intersect(&a_obj.transform, &b_obj.transform, a_coll, b_coll) {
        let collision = Collision {
            a: crate::EntityPayload {
                entity: a.entity,
                is_trigger: a_obj.is_trigger,
                is_static: a_obj.is_static,
            },
            b: crate::EntityPayload {
                entity: b.entity,
                is_trigger: b_obj.is_trigger,
                is_static: b_obj.is_static,
            },
            contact,
        };

        Some(collision)
    } else {
        None
    }
}

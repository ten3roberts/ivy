use hecs::Column;
use ivy_base::{Color, DrawGizmos, Events};
use ordered_float::OrderedFloat;
use slotmap::SlotMap;
use smallvec::{smallvec, SmallVec};
use ultraviolet::Vec3;

use crate::{
    intersect, BoundingBox, Collider, Collision, CollisionTreeNode, NodeIndex, Nodes, Object,
    ObjectData, ObjectIndex,
};

const MARGIN: f32 = 1.2;

type Objects = SmallVec<[Object; 16]>;

#[derive(Debug, Clone)]
pub struct BVHNode {
    bounds: BoundingBox,
    objects: Objects,
    axis: Axis,
    children: Option<[NodeIndex; 2]>,
    depth: u32,
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

impl BVHNode {
    pub fn new(bounds: BoundingBox, axis: Axis) -> Self {
        Self {
            bounds,
            objects: Objects::new(),
            axis,
            children: None,
            depth: 0,
        }
    }

    fn from_objects(
        nodes: &mut Nodes<Self>,
        objects: Objects,
        data: &SlotMap<ObjectIndex, ObjectData>,
        axis: Axis,
        depth: u32,
    ) -> NodeIndex {
        let bounds = Self::calculate_bounds(&objects, data);

        let node = Self {
            bounds,
            objects: objects.into(),
            axis,
            children: None,
            depth,
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
        let left = node.objects[0..median].into();
        let right = node.objects[median..].into();
        let new_axis = node.axis.rotate();
        let depth = node.depth + 1;

        node.objects.clear();

        let left = Self::from_objects(nodes, left, data, new_axis, depth);
        let right = Self::from_objects(nodes, right, data, new_axis, depth);

        nodes[index].children = Some([left, right]);
    }

    pub fn calculate_bounds_incremental(&self, object: &ObjectData) -> BoundingBox {
        self.bounds.merge(
            object
                .bounds
                .rel_margin(if object.is_static { 1.0 } else { MARGIN }),
        )
    }

    /// Updates the bounds of the object
    pub fn calculate_bounds(
        objects: &[Object],
        data: &SlotMap<ObjectIndex, ObjectData>,
    ) -> BoundingBox {
        let lx = objects
            .iter()
            .map(|val| OrderedFloat(data[val.index].bounds.neg_x()))
            .min()
            .unwrap_or_default();

        let ly = objects
            .iter()
            .map(|val| OrderedFloat(data[val.index].bounds.neg_y()))
            .min()
            .unwrap_or_default();

        let lz = objects
            .iter()
            .map(|val| OrderedFloat(data[val.index].bounds.neg_z()))
            .min()
            .unwrap_or_default();

        let rx = objects
            .iter()
            .map(|val| OrderedFloat(data[val.index].bounds.x()))
            .max()
            .unwrap_or_default();

        let ry = objects
            .iter()
            .map(|val| OrderedFloat(data[val.index].bounds.y()))
            .max()
            .unwrap_or_default();

        let rz = objects
            .iter()
            .map(|val| OrderedFloat(data[val.index].bounds.z()))
            .max()
            .unwrap_or_default();

        BoundingBox::from_corners(Vec3::new(*lx, *ly, *lz), Vec3::new(*rx, *ry, *rz))
            .rel_margin(MARGIN)
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
    fn collapse(index: NodeIndex, nodes: &mut Nodes<Self>, objects: &mut Objects) {
        let node = &mut nodes[index];

        objects.append(&mut node.objects);

        if let Some([l, r]) = node.children.take() {
            Self::collapse(l, nodes, objects);
            Self::collapse(r, nodes, objects);
            nodes.remove(l).unwrap();
            nodes.remove(r).unwrap();
        }
    }
}

impl CollisionTreeNode for BVHNode {
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

                node.bounds = Self::calculate_bounds(&objects, data);
                node.objects = objects;
                Self::try_split(index, nodes, data);
            }
        } else {
            node.objects.push(object);

            // Split
            Self::try_split(index, nodes, data);
        }
    }

    fn remove(&mut self, e: hecs::Entity) -> Option<Object> {
        if let Some(idx) = self.objects.iter().position(|val| val.entity == e) {
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
        let node = &mut nodes[index];

        let bounds = node.bounds;

        let mut removed = 0;

        node.objects.retain(|val| {
            let obj = data[val.index];

            if bounds.contains(obj.bounds) {
                true
            } else {
                removed += 1;
                to_refit.push(*val);
                false
            }
        });

        if removed > 0 {
            node.bounds = Self::calculate_bounds(&node.objects, data);
        }

        if let Some([left, right]) = node.children {
            Self::update(left, nodes, data, to_refit);
            Self::update(right, nodes, data, to_refit);
        }
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
        } else {
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

impl DrawGizmos for BVHNode {
    fn draw_gizmos<T: std::ops::DerefMut<Target = ivy_base::Gizmos>>(
        &self,
        mut gizmos: T,
        _: Color,
    ) {
        let color = Color::hsl(
            self.depth as f32 * 20.0,
            1.0,
            if self.is_leaf() { 0.5 } else { 0.1 },
        );

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
                is_static: a_obj.is_static,
            },
            contact,
        };

        Some(collision)
    } else {
        None
    }
}

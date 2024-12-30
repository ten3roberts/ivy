use bytemuck::Zeroable;
use flax::{
    component,
    components::child_of,
    fetch::{entity_refs, Copied, Modified, Source, TransformFetch, Traverse},
    filter::{All, With},
    Component, Entity, Fetch, FetchExt, Query, World,
};
use glam::Mat4;
use ivy_core::{
    components::world_transform, profiling::profile_function,
    subscribers::RemovedComponentSubscriber, WorldExt,
};
use ivy_wgpu_types::{BindGroupBuilder, BindGroupLayoutBuilder, Gpu, TypedBuffer};
use wgpu::{BindGroupLayout, BufferUsages, ShaderStages};

use crate::components::mesh;

#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct RenderObjectData {
    transform: Mat4,
}

impl RenderObjectData {
    pub fn new(transform: Mat4) -> Self {
        Self { transform }
    }
}

#[derive(Fetch)]
#[fetch(transforms = [ Modified ])]
struct ObjectDataQuery {
    transform: Source<Copied<Component<Mat4>>, Traverse>,
}

impl ObjectDataQuery {
    pub fn new() -> Self {
        Self {
            transform: world_transform().copied().traverse(child_of),
        }
    }
}

type UpdateFetch = (
    Component<usize>,
    <ObjectDataQuery as TransformFetch<Modified>>::Output,
);

pub struct ObjectManager {
    object_data: Vec<RenderObjectData>,
    object_map: Vec<Entity>,

    object_buffer: TypedBuffer<RenderObjectData>,
    bind_group_layout: BindGroupLayout,
    bind_group: wgpu::BindGroup,
    removed_rx: flume::Receiver<(flax::Entity, usize)>,
    object_query: Query<UpdateFetch, (All, With)>,
}

impl ObjectManager {
    pub fn new(world: &mut World, gpu: &Gpu) -> Self {
        let bind_group_layout = BindGroupLayoutBuilder::new("ObjectBuffer")
            .bind_storage_buffer(ShaderStages::VERTEX)
            .build(gpu);

        let object_buffer = TypedBuffer::new(
            gpu,
            "Object buffer",
            BufferUsages::STORAGE | BufferUsages::COPY_DST,
            &[RenderObjectData::zeroed(); 64],
        );

        let bind_group = BindGroupBuilder::new("ObjectBuffer")
            .bind_buffer(&object_buffer)
            .build(gpu, &bind_group_layout);

        let (removed_tx, removed_rx) = flume::unbounded();
        world.subscribe(RemovedComponentSubscriber::new(
            removed_tx,
            object_buffer_index(),
        ));

        Self {
            object_data: Vec::new(),
            object_map: Vec::new(),
            object_buffer,
            bind_group_layout,
            bind_group,
            removed_rx,
            object_query: Query::new((object_buffer_index(), ObjectDataQuery::new().modified()))
                .with(mesh()),
        }
    }

    fn resize_object_buffer(&mut self, gpu: &Gpu, capacity: usize) {
        if self.object_buffer.len() >= capacity {
            return;
        }

        self.object_buffer
            .resize(gpu, capacity.next_power_of_two(), false);

        self.bind_group = BindGroupBuilder::new("ObjectBuffer")
            .bind_buffer(&self.object_buffer)
            .build(gpu, &self.bind_group_layout);
    }

    pub fn collect_unbatched(&mut self, world: &mut World, gpu: &Gpu) {
        profile_function!();
        let mut query = Query::new((entity_refs(), ObjectDataQuery::new()))
            .with(mesh())
            .without(object_buffer_index());

        let mut new_components = Vec::new();

        for (entity, item) in &mut query.borrow(world) {
            let id = entity.id();
            let new_index = self.object_data.len();
            new_components.push((id, new_index));

            tracing::info!(%entity, %new_index, "new object");
            self.object_data.push(RenderObjectData::new(item.transform));
            self.object_map.push(id);
        }

        world
            .append_all(object_buffer_index(), new_components)
            .unwrap();

        if self.object_data.len() > self.object_buffer.len() {
            self.resize_object_buffer(gpu, self.object_data.len());
        }

        {
            self.object_buffer.write(&gpu.queue, 0, &self.object_data);
        }
    }

    pub fn process_removed(&mut self, world: &World) {
        profile_function!();
        for (id, loc) in self.removed_rx.try_iter() {
            if loc == self.object_data.len() - 1 {
                assert!(self.object_map[loc] == id);
                self.object_map.pop();
                self.object_data.pop();
            } else {
                let end = self.object_data.len() - 1;
                self.object_data.swap(loc, end);
                self.object_map.swap(loc, end);

                let swapped_entity = self.object_map[loc];

                let _ = world.update_dedup(swapped_entity, object_buffer_index(), loc);
            }
        }
    }

    fn update_object_data(&mut self, world: &World, gpu: &Gpu) {
        profile_function!();
        for (&loc, item) in &mut self.object_query.borrow(world) {
            assert_ne!(loc, usize::MAX);
            self.object_data[loc] = RenderObjectData {
                transform: item.transform,
            };
        }

        self.object_buffer.write(&gpu.queue, 0, &self.object_data);
    }

    pub fn update(&mut self, world: &mut World, gpu: &Gpu) -> anyhow::Result<()> {
        profile_function!();
        self.process_removed(world);
        self.collect_unbatched(world, gpu);
        self.update_object_data(world, gpu);

        Ok(())
    }

    pub fn object_buffer(&self) -> &TypedBuffer<RenderObjectData> {
        &self.object_buffer
    }

    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }

    pub fn bind_group_layout(&self) -> &BindGroupLayout {
        &self.bind_group_layout
    }
}

component! {
    pub(crate) object_manager: ObjectManager,
    pub(crate) object_buffer_index: usize,
}

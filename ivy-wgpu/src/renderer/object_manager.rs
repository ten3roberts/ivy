use bytemuck::Zeroable;
use flax::{
    component,
    components::child_of,
    fetch::{entity_refs, Copied, Modified, Source, TransformFetch, Traverse},
    filter::{All, With},
    Component, Entity, Fetch, FetchExt, Query, World,
};
use glam::{Mat4, Vec3};
use ivy_assets::Asset;
use ivy_core::{
    components::{color, world_transform},
    palette::WithAlpha,
    profiling::profile_function,
    subscribers::RemovedComponentSubscriber,
    to_linear_vec3, Color, WorldExt,
};
use ivy_gltf::{
    animation::{player::Animator, skin::Skin},
    components::{animator, skin},
};
use ivy_wgpu_types::{
    multi_buffer::{MultiBuffer, SubBuffer},
    BindGroupBuilder, BindGroupLayoutBuilder, Gpu, TypedBuffer,
};
use wgpu::{BindGroupLayout, BufferUsages, ShaderStages};

use crate::components::mesh;

#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct RenderObjectData {
    transform: Mat4,
    color: Vec3,
    joint_offset: u32,
}

impl RenderObjectData {
    pub fn new(transform: Mat4, joint_offset: Option<u32>, color: Vec3) -> Self {
        Self {
            transform,
            joint_offset: joint_offset.unwrap_or(u32::MAX),
            color,
        }
    }
}

#[derive(Fetch)]
#[fetch(transforms = [ Modified ])]
struct ObjectDataQuery {
    transform: Source<Copied<Component<Mat4>>, Traverse>,
    color: Source<Copied<Component<Color>>, Traverse>,
}

impl ObjectDataQuery {
    pub fn new() -> Self {
        Self {
            transform: world_transform().copied().traverse(child_of),
            color: color().copied().traverse(child_of),
        }
    }
}

type UpdateFetch = (
    Component<usize>,
    <ObjectDataQuery as TransformFetch<Modified>>::Output,
);

type SkinUpdateFetch = (
    Component<usize>,
    Component<SubBuffer<Mat4>>,
    Source<
        (
            Component<Asset<Skin>>,
            <Component<Animator> as TransformFetch<Modified>>::Output,
        ),
        Traverse,
    >,
);
pub struct ObjectManager {
    object_data: Vec<RenderObjectData>,
    object_map: Vec<Entity>,

    object_buffer: TypedBuffer<RenderObjectData>,

    skinning_buffer: MultiBuffer<Mat4>,
    skinning_data: Vec<Mat4>,

    bind_group_layout: BindGroupLayout,
    bind_group: wgpu::BindGroup,
    removed_rx: flume::Receiver<(flax::Entity, usize)>,
    object_query: Query<UpdateFetch, (All, With)>,
    skin_query: Query<SkinUpdateFetch, (All, With)>,
}

impl ObjectManager {
    pub fn new(world: &mut World, gpu: &Gpu) -> Self {
        let bind_group_layout = BindGroupLayoutBuilder::new("ObjectBuffer")
            .bind_storage_buffer(ShaderStages::VERTEX)
            .bind_storage_buffer(ShaderStages::VERTEX)
            .build(gpu);

        let object_buffer = TypedBuffer::new(
            gpu,
            "object_buffer",
            BufferUsages::STORAGE | BufferUsages::COPY_DST,
            &[RenderObjectData::zeroed(); 64],
        );

        let skinning_buffer = MultiBuffer::new(
            gpu,
            "skinning_buffer",
            BufferUsages::STORAGE | BufferUsages::COPY_DST,
            64,
        );

        let bind_group = BindGroupBuilder::new("ObjectBuffer")
            .bind_buffer(&object_buffer)
            .bind_buffer(skinning_buffer.buffer())
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
            skin_query: Query::new((
                object_buffer_index(),
                object_skinning_buffer(),
                (skin(), animator().modified()).traverse(child_of),
            ))
            .with(mesh()),
            skinning_data: vec![Mat4::IDENTITY; skinning_buffer.len()],
            skinning_buffer,
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
            .bind_buffer(self.skinning_buffer.buffer())
            .build(gpu, &self.bind_group_layout);
    }

    pub fn collect_unbatched(&mut self, world: &mut World, gpu: &Gpu) {
        profile_function!();
        let mut query = Query::new((
            entity_refs(),
            (world_transform(), skin().opt()).traverse(child_of),
        ))
        .with(mesh())
        .without(object_buffer_index());

        let mut new_components = Vec::new();
        let mut new_skin_components = Vec::new();

        let mut resized_skinning_buffer = false;
        for (entity, (&transform, skin)) in &mut query.borrow(world) {
            let id = entity.id();
            let new_index = self.object_data.len();

            let skin_buffer_offset = match skin {
                Some(skin) => {
                    let joints = skin.joints().len();
                    let subbuffer = if let Some(handle) = self.skinning_buffer.allocate(joints) {
                        handle
                    } else {
                        resized_skinning_buffer = true;
                        self.skinning_buffer.grow(gpu, joints);
                        self.skinning_data
                            .resize(self.skinning_buffer.len(), Mat4::IDENTITY);

                        self.skinning_buffer.allocate(joints).unwrap()
                    };

                    new_skin_components.push((id, subbuffer));
                    Some(subbuffer.offset() as u32)
                }
                None => None,
            };

            new_components.push((id, new_index));

            self.object_data.push(RenderObjectData::new(
                transform,
                skin_buffer_offset,
                Vec3::ONE,
            ));

            self.object_map.push(id);
        }

        world
            .append_all(object_buffer_index(), new_components)
            .unwrap();

        world
            .append_all(object_skinning_buffer(), new_skin_components)
            .unwrap();

        if self.object_data.len() > self.object_buffer.len() {
            self.resize_object_buffer(gpu, self.object_data.len());
        } else if resized_skinning_buffer {
            self.bind_group = BindGroupBuilder::new("ObjectBuffer")
                .bind_buffer(&self.object_buffer)
                .bind_buffer(self.skinning_buffer.buffer())
                .build(gpu, &self.bind_group_layout);
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
            let object_data = &mut self.object_data[loc];
            object_data.transform = item.transform;
            object_data.color = to_linear_vec3(item.color.without_alpha())
        }

        self.object_buffer.write(&gpu.queue, 0, &self.object_data);
    }

    fn update_skin_data(&mut self, world: &World, gpu: &Gpu) {
        profile_function!();
        for (&loc, skin_buffer, (skin, animator)) in &mut self.skin_query.borrow(world) {
            assert_ne!(loc, usize::MAX);
            let object_data = &mut self.object_data[loc];

            let data = &mut self.skinning_data[object_data.joint_offset as usize
                ..object_data.joint_offset as usize + skin.joints().len()];
            animator.fill_buffer(skin, data);

            self.skinning_buffer.write(&gpu.queue, skin_buffer, data);
        }
    }

    pub fn update(&mut self, world: &mut World, gpu: &Gpu) -> anyhow::Result<()> {
        profile_function!();
        self.process_removed(world);
        self.collect_unbatched(world, gpu);
        self.update_object_data(world, gpu);
        self.update_skin_data(world, gpu);

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

// pub struct ObjectBufferLoc {
//     index: usize,
//     skin_buffer_offset: Option<SubBuffer<Mat4>>,
// }

component! {
    pub(crate) object_buffer_index: usize,
    pub(crate) object_skinning_buffer: SubBuffer<Mat4>,
}

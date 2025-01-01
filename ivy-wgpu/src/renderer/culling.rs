use std::convert::Infallible;

use bytemuck::{NoUninit, Pod, Zeroable};
use flax::Entity;
use glam::{Mat4, Vec4};
use ivy_assets::{Asset, AssetCache, AssetDesc};
use ivy_core::profiling::profile_function;
use ivy_wgpu_types::{BindGroupBuilder, BindGroupLayoutBuilder, Gpu, TypedBuffer};
use wgpu::{
    BindGroup, BindGroupLayout, BufferUsages, CommandEncoder, ComputePassDescriptor,
    ComputePipeline, ComputePipelineDescriptor, PipelineLayoutDescriptor, ShaderStages,
};

use super::{mesh_renderer::DrawIndexedIndirectArgs, object_manager::RenderObjectData};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct ObjectCullingPipelineDesc;

impl AssetDesc<ComputePipeline> for ObjectCullingPipelineDesc {
    type Error = Infallible;

    fn create(
        &self,
        assets: &ivy_assets::AssetCache,
    ) -> Result<ivy_assets::Asset<ComputePipeline>, Self::Error> {
        let gpu = &*assets.service();
        let layout = BindGroupLayoutBuilder::new("ObjectCulling")
            .bind_uniform_buffer(ShaderStages::COMPUTE) // cull_data
            .bind_storage_buffer(ShaderStages::COMPUTE) // object_data
            .bind_storage_buffer(ShaderStages::COMPUTE) // draws
            .bind_storage_buffer_write(ShaderStages::COMPUTE) // indirect_draws
            .bind_storage_buffer_write(ShaderStages::COMPUTE) // indirection_buffer
            .build(gpu);

        let pipeline_layout = gpu
            .device
            .create_pipeline_layout(&PipelineLayoutDescriptor {
                label: Some("ObjectCulling"),
                bind_group_layouts: &[&layout],
                push_constant_ranges: &[],
            });

        let pipeline = gpu
            .device
            .create_compute_pipeline(&ComputePipelineDescriptor {
                label: Some("ObjectCulling"),
                layout: Some(&pipeline_layout),
                module: &gpu
                    .device
                    .create_shader_module(wgpu::ShaderModuleDescriptor {
                        label: Some("ObjectCulling"),
                        source: wgpu::ShaderSource::Wgsl(
                            include_str!("../../../assets/shaders/object_culling.wgsl").into(),
                        ),
                    }),
                entry_point: "main",
                compilation_options: Default::default(),
                cache: None,
            });

        Ok(assets.insert(pipeline))
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, Pod, Zeroable)]
pub struct CullData {
    pub view: Mat4,
    pub frustum: Vec4,
    pub near: f32,
    pub far: f32,
    pub object_count: u32,
    pub _padding: f32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, NoUninit)]
pub struct CullDrawObject {
    pub object_index: u32,
    pub batch_id: u32,
    pub radius: f32,
    pub id: Entity,
}

pub(crate) struct ObjectCulling {
    pipeline: Asset<ComputePipeline>,
    bind_group_layout: BindGroupLayout,
    pub(crate) bind_group: Option<BindGroup>,
    // Maps the batch instance id to the object index
    cull_data_buffer: TypedBuffer<CullData>,
    draw_object_buffer: TypedBuffer<CullDrawObject>,
    indirect_draw_buffer: TypedBuffer<DrawIndexedIndirectArgs>,
    indirection_buffer: TypedBuffer<u32>,
}

impl ObjectCulling {
    pub fn new(assets: &AssetCache, gpu: &Gpu) -> Self {
        let pipeline = assets.load(&ObjectCullingPipelineDesc);

        let cull_data_buffer = TypedBuffer::new_uninit(
            gpu,
            "CullData",
            BufferUsages::INDIRECT | BufferUsages::COPY_DST | BufferUsages::UNIFORM,
            1,
        );

        let draw_object_buffer = TypedBuffer::new_uninit(
            gpu,
            "DrawObject",
            BufferUsages::INDIRECT | BufferUsages::COPY_DST | BufferUsages::STORAGE,
            128,
        );

        let indirect_draw_buffer = TypedBuffer::new_uninit(
            gpu,
            "IndirectDrawBuffer",
            BufferUsages::INDIRECT | BufferUsages::COPY_DST | BufferUsages::STORAGE,
            128,
        );

        let indirection_buffer = TypedBuffer::new_uninit(
            gpu,
            "IndirectionBuffer",
            BufferUsages::INDIRECT | BufferUsages::COPY_DST | BufferUsages::STORAGE,
            128,
        );

        Self {
            bind_group_layout: pipeline.get_bind_group_layout(0),
            pipeline,
            bind_group: None,
            cull_data_buffer,
            draw_object_buffer,
            indirection_buffer,
            indirect_draw_buffer,
        }
    }

    pub fn update_objects(&mut self, gpu: &Gpu, draw_objects: &[CullDrawObject]) {
        if self.draw_object_buffer.len() < draw_objects.len() {
            self.indirection_buffer
                .resize(gpu, draw_objects.len().next_power_of_two(), false);
            self.draw_object_buffer
                .resize(gpu, draw_objects.len().next_power_of_two(), false);

            self.bind_group = None;
        }

        self.draw_object_buffer.write(&gpu.queue, 0, draw_objects);
    }

    pub fn run(
        &mut self,
        gpu: &Gpu,
        encoder: &mut CommandEncoder,
        cull_data: CullData,
        object_buffer: &TypedBuffer<RenderObjectData>,
        indirect_draws: &[DrawIndexedIndirectArgs],
    ) {
        profile_function!();
        if self.indirect_draw_buffer.len() < indirect_draws.len() {
            self.indirect_draw_buffer
                .resize(gpu, indirect_draws.len(), false);

            self.bind_group = None;
        }

        self.indirect_draw_buffer
            .write(&gpu.queue, 0, indirect_draws);

        self.cull_data_buffer.write(&gpu.queue, 0, &[cull_data]);
        let mut compute_pass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("culling"),
            timestamp_writes: None,
        });

        let bind_group = self.bind_group.get_or_insert_with(|| {
            BindGroupBuilder::new("ObjectCulling")
                .bind_buffer(&self.cull_data_buffer)
                .bind_buffer(object_buffer)
                .bind_buffer(&self.draw_object_buffer)
                .bind_buffer(&self.indirect_draw_buffer)
                .bind_buffer(&self.indirection_buffer)
                .build(gpu, &self.bind_group_layout)
        });

        compute_pass.set_pipeline(&self.pipeline);
        compute_pass.set_bind_group(0, bind_group, &[]);
        compute_pass.dispatch_workgroups(cull_data.object_count.div_ceil(256), 1, 1);
    }

    pub(crate) fn indirection_buffer(&self) -> &TypedBuffer<u32> {
        &self.indirection_buffer
    }

    pub(crate) fn indirect_draw_buffer(&self) -> &TypedBuffer<DrawIndexedIndirectArgs> {
        &self.indirect_draw_buffer
    }
}

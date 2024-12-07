use std::{mem::size_of, sync::Arc};

use crate::{
    components::{cast_shadow, light_kind, light_params, projection_matrix, shadow_pass},
    light::{LightKind, LightParams},
    renderer::{
        mesh_renderer::MeshRenderer, CameraRenderer, RenderContext, RendererStore, UpdateContext,
    },
    rendergraph::{
        BufferHandle, Dependency, Node, NodeExecutionContext, NodeUpdateContext, TextureHandle,
        UpdateResult,
    },
    shader_library::ShaderLibrary,
    types::{shader::TargetDesc, BindGroupBuilder, BindGroupLayoutBuilder},
    Gpu,
};
use flax::{entity_ids, filter::With, Component, EntityIds, FetchExt, Query, World};
use glam::{vec2, vec3, Mat4, Vec2, Vec3, Vec4Swizzles};
use itertools::{izip, Itertools};
use ivy_core::{
    components::{main_camera, world_transform},
    profiling::{profile_function, profile_scope},
    WorldExt,
};
use ivy_wgpu_types::shader::ShaderDesc;
use ordered_float::OrderedFloat;
use tracing::info;
use wgpu::{
    BindGroup, BindGroupLayout, Buffer, BufferDescriptor, BufferUsages, DepthBiasState, Operations,
    RenderPassDescriptor, ShaderStages, TextureUsages, TextureView, TextureViewDescriptor,
    TextureViewDimension,
};

use crate::components::light_shadow_data;

use super::skinned_mesh_renderer::SkinnedMeshRenderer;

#[derive(Debug, Clone, Copy)]
pub struct LightShadowData {
    pub index: u32,
    pub cascade_count: u32,
}

#[repr(C)]
#[derive(bytemuck::Pod, bytemuck::Zeroable, Clone, Copy, Debug)]
pub struct LightShadowCamera {
    viewproj: Mat4,
    texel_size: Vec2,
    depth: f32,
    _padding: f32,
}

#[derive(flax::Fetch)]
struct ShadowMapNodeQuery {
    id: EntityIds,
    world_transform: Component<Mat4>,
    light_kind: Component<LightKind>,
    light_params: Component<LightParams>,
    cast_shadow: With,
}

pub struct ShadowMapNode {
    // Texture array
    shadow_maps: TextureHandle,
    shadow_map_views: Option<Vec<TextureView>>,
    layout: BindGroupLayout,
    bind_groups: Option<Vec<BindGroup>>,
    lights: Vec<LightShadowCamera>,
    dynamic_light_camera_buffer: Buffer,
    shadow_camera_buffer: BufferHandle,
    renderer: (MeshRenderer, SkinnedMeshRenderer),
    store: RendererStore,
    max_cascades: usize,
    query: Query<ShadowMapNodeQuery>,
}

impl ShadowMapNode {
    pub fn new(
        world: &mut World,
        gpu: &Gpu,
        shadow_maps: TextureHandle,
        light_camera_buffer: BufferHandle,
        max_shadows: usize,
        max_cascades: usize,
        shader_library: Arc<ShaderLibrary>,
    ) -> Self {
        let layout = BindGroupLayoutBuilder::new("LightCameraBuffer")
            .bind_uniform_buffer(ShaderStages::VERTEX)
            .build(gpu);

        fn shader_factory(desc: ShaderDesc) -> ShaderDesc {
            desc.with_depth_bias(DepthBiasState {
                constant: 2,
                slope_scale: 2.0,
                clamp: 0.0,
            })
        }

        let renderer = (
            MeshRenderer::new(world, gpu, shadow_pass(), shader_library.clone())
                .with_shader_factory(shader_factory),
            SkinnedMeshRenderer::new(world, gpu, shadow_pass(), shader_library)
                .with_shader_factory(shader_factory),
        );

        let align = gpu.device.limits().min_uniform_buffer_offset_alignment as u64;

        let dynamic_light_camera_buffer = gpu.device.create_buffer(&BufferDescriptor {
            label: Some("dynamic_light_camera_buffer"),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            size: (align.max(size_of::<LightShadowCamera>() as u64)
                * max_shadows as u64
                * max_cascades as u64),
            mapped_at_creation: false,
        });

        Self {
            shadow_maps,
            max_cascades,
            layout,
            bind_groups: None,
            lights: vec![],
            dynamic_light_camera_buffer,
            shadow_camera_buffer: light_camera_buffer,
            renderer,
            store: RendererStore::new(),
            query: Query::new(ShadowMapNodeQuery {
                id: entity_ids(),
                world_transform: world_transform(),
                light_kind: light_kind(),
                light_params: light_params(),
                cast_shadow: cast_shadow().with(),
            }),
            shadow_map_views: None,
        }
    }
}

impl Node for ShadowMapNode {
    fn update(&mut self, ctx: NodeUpdateContext) -> anyhow::Result<UpdateResult> {
        ivy_core::profiling::profile_function!();

        let shadow_maps = ctx.get_texture(self.shadow_maps);
        let texel_size = vec2(shadow_maps.width() as f32, shadow_maps.height() as f32).recip();

        let mut to_add = Vec::new();

        self.lights.clear();

        let Some(main_camera) =
            Query::new((world_transform().copied(), projection_matrix().copied()))
                .with(main_camera())
                .borrow(ctx.world)
                .first()
        else {
            tracing::warn!("no main camera");
            return Ok(UpdateResult::Success);
        };

        let camera_inv_viewproj = main_camera.0 * main_camera.1.inverse();

        fn transform_perspective(inv_viewproj: Mat4, clip: Vec3) -> Vec3 {
            let p = inv_viewproj * clip.extend(1.0);
            p.xyz() / p.w
        }

        let near = -transform_perspective(main_camera.1.inverse(), Vec3::ZERO).z;
        let far = -transform_perspective(main_camera.1.inverse(), Vec3::Z).z;

        let clip_range = far - near;

        let min_z = near;
        let max_z = near + clip_range;

        let range = max_z - min_z;
        let ratio = max_z / min_z;
        let cascade_split_lambda = 0.95;

        let clip_distances = (0..self.max_cascades)
            .map(|i| {
                let p = (i + 1) as f32 / self.max_cascades as f32;

                let log = min_z * ratio.powf(p);
                let uniform = min_z + range * p;
                let d = cascade_split_lambda * (log - uniform) + uniform;
                (d - near) / clip_range
            })
            .collect_vec();

        let mut last_split_distance = 0.0;
        let frustrums = (0..self.max_cascades)
            .map(|i| {
                let frustrum = Frustrum::from_inv_viewproj(
                    camera_inv_viewproj,
                    near,
                    clip_range,
                    clip_distances[i],
                    last_split_distance,
                );

                last_split_distance = clip_distances[i];

                frustrum
            })
            .collect_vec();

        for item in self.query.borrow(ctx.world).iter() {
            profile_scope!("update_light_camera");
            let (_, light_rot, light_pos) = item.world_transform.to_scale_rotation_translation();

            let light_index = self.lights.len() as u32;

            let light_forward = light_rot * -Vec3::Z;
            if item.light_kind.is_directional() {
                for frustrum in &frustrums {
                    let snapping = 0.1;
                    let center = (frustrum.center / snapping).ceil() * snapping;

                    let radius = frustrum
                        .corners
                        .iter()
                        .map(|v| v.distance(center))
                        .max_by_key(|&v| OrderedFloat(v))
                        .unwrap();

                    let radius = (radius / snapping).ceil() * snapping + snapping;

                    let light_camera_pos = center - light_forward * radius;
                    let view = Mat4::from_rotation_translation(light_rot, light_camera_pos);

                    let proj =
                        Mat4::orthographic_rh(-radius, radius, -radius, radius, 0.1, radius * 2.0);

                    self.lights.push(LightShadowCamera {
                        viewproj: proj * view.inverse(),
                        texel_size,
                        depth: frustrum.split_distance,
                        _padding: Default::default(),
                    });

                    to_add.push((
                        item.id,
                        LightShadowData {
                            index: light_index,
                            cascade_count: self.max_cascades as u32,
                        },
                    ));
                }
            } else {
                let view = Mat4::from_rotation_translation(light_rot, light_pos);

                const MIN_LUM: f32 = 0.01;
                let max_range = (item.light_params.intensity / MIN_LUM).sqrt();

                let proj =
                    Mat4::perspective_rh(item.light_params.outer_theta * 2.0, 1.0, 0.1, max_range);

                self.lights.push(LightShadowCamera {
                    viewproj: proj * view.inverse(),
                    texel_size,
                    depth: 0.0,
                    _padding: Default::default(),
                });

                to_add.push((
                    item.id,
                    LightShadowData {
                        index: light_index,
                        cascade_count: 1,
                    },
                ));
            };
        }

        if self
            .shadow_map_views
            .as_ref()
            .is_some_and(|v| v.len() != self.lights.len())
        {
            self.shadow_map_views = None;
            self.bind_groups = None;
        }

        ctx.world.append_all(light_shadow_data(), to_add)?;

        let mut update_ctx = UpdateContext {
            world: ctx.world,
            assets: ctx.assets,
            gpu: ctx.gpu,
            store: &mut self.store,
            layouts: &[&self.layout],
            camera: Default::default(),
            target_desc: TargetDesc {
                formats: &[],
                depth_format: Some(shadow_maps.format()),
                sample_count: shadow_maps.sample_count(),
            },
        };

        self.renderer.update(&mut update_ctx)?;

        Ok(UpdateResult::Success)
    }

    fn draw(&mut self, ctx: NodeExecutionContext) -> anyhow::Result<()> {
        profile_function!();
        let shadow_maps = ctx.get_texture(self.shadow_maps);

        let light_shadow_stride = (ctx.gpu.device.limits().min_uniform_buffer_offset_alignment
            as u64)
            .max(size_of::<LightShadowCamera>() as u64);

        {
            profile_scope!("write_light_buffer");
            ctx.queue.write_buffer(
                ctx.get_buffer(self.shadow_camera_buffer),
                0,
                bytemuck::cast_slice(&self.lights),
            );

            for (i, &light) in self.lights.iter().enumerate() {
                ctx.queue.write_buffer(
                    &self.dynamic_light_camera_buffer,
                    i as u64 * light_shadow_stride,
                    bytemuck::cast_slice(&[light]),
                );
            }
        }
        let bind_groups = self.bind_groups.get_or_insert_with(|| {
            profile_scope!("create_bind_groups");

            (0..self.lights.len())
                .map(|i| {
                    let bind_group = BindGroupBuilder::new("LightCamera")
                        .bind_buffer_slice(
                            &self.dynamic_light_camera_buffer,
                            i as u64 * light_shadow_stride,
                            size_of::<LightShadowCamera>() as u64,
                        )
                        .build(ctx.gpu, &self.layout);

                    bind_group
                })
                .collect_vec()
        });

        let shadow_map_views = self.shadow_map_views.get_or_insert_with(|| {
            profile_scope!("create_shadow_views");

            (0..self.lights.len())
                .map(|i| {
                    shadow_maps.create_view(&TextureViewDescriptor {
                        aspect: wgpu::TextureAspect::DepthOnly,
                        dimension: Some(TextureViewDimension::D2),
                        base_array_layer: i as u32,
                        array_layer_count: Some(1),
                        ..Default::default()
                    })
                })
                .collect_vec()
        });

        assert_eq!(bind_groups.len(), shadow_map_views.len());
        for (bind_group, view) in izip!(bind_groups, shadow_map_views) {
            profile_scope!("cascade_draw");

            let draw_ctx = RenderContext {
                world: ctx.world,
                assets: ctx.assets,
                gpu: ctx.gpu,
                queue: ctx.queue,
                store: &mut self.store,
                bind_groups: &[bind_group],
                layouts: &[&self.layout],
                target_desc: TargetDesc {
                    formats: &[],
                    depth_format: Some(shadow_maps.format()),
                    sample_count: shadow_maps.sample_count(),
                },
            };

            let mut render_pass = {
                profile_scope!("begin_render_pass");
                ctx.encoder.begin_render_pass(&RenderPassDescriptor {
                    label: "shadow_map".into(),
                    color_attachments: &[],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view,
                        depth_ops: Some(Operations {
                            load: wgpu::LoadOp::Clear(1.0),
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),
                    ..Default::default()
                })
            };

            self.renderer.draw(&draw_ctx, &mut render_pass)?;
        }

        Ok(())
    }

    fn read_dependencies(&self) -> Vec<Dependency> {
        vec![]
    }

    fn write_dependencies(&self) -> Vec<Dependency> {
        vec![
            Dependency::texture(self.shadow_maps, TextureUsages::RENDER_ATTACHMENT),
            Dependency::buffer(self.shadow_camera_buffer, BufferUsages::COPY_DST),
        ]
    }

    fn on_resource_changed(&mut self, _resource: crate::rendergraph::ResourceHandle) {
        self.bind_groups = None;
        self.shadow_map_views = None;
    }
}

struct Frustrum {
    corners: [Vec3; 8],
    center: Vec3,
    split_distance: f32,
}

impl Frustrum {
    fn from_inv_viewproj(
        inv: Mat4,
        near: f32,
        clip_range: f32,
        split_distance: f32,
        last_split_distance: f32,
    ) -> Self {
        let mut corners = [Vec3::ZERO; 8];
        let mut corner_directions = [
            vec3(-1.0, 1., 0.),
            vec3(1.0, 1., 0.),
            vec3(1.0, -1., 0.),
            vec3(-1.0, -1., 0.),
            vec3(-1.0, 1., 1.),
            vec3(1.0, 1., 1.),
            vec3(1.0, -1., 1.),
            vec3(-1.0, -1., 1.),
        ];

        for (dir, corner) in corner_directions.iter_mut().zip(&mut corners) {
            let point = inv * dir.extend(1.0);
            let point = point.xyz() / point.w;
            *corner = point;
        }

        for i in 0..4 {
            let dist = corners[i + 4] - corners[i];
            corners[i + 4] = corners[i] + (dist * split_distance);
            corners[i] += dist * last_split_distance;
        }

        let center = corners.iter().sum::<Vec3>() / corners.len() as f32;

        let depth = -(near + split_distance * clip_range);
        Self {
            corners,
            center,
            split_distance: depth,
        }
    }
}

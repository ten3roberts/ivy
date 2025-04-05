use std::{mem::size_of, sync::Arc};

use flax::{entity_ids, filter::With, Component, EntityIds, Query};
use glam::{vec2, vec3, Mat4, Vec2, Vec3, Vec4Swizzles};
use itertools::{izip, Itertools};
use ivy_assets::stored::Handle;
use ivy_core::{
    components::{main_camera, world_transform},
    math::Vec3Ext,
    profiling::{profile_function, profile_scope},
    WorldExt,
};
use ivy_wgpu_types::shader::ShaderDesc;
use ordered_float::OrderedFloat;
use wgpu::{
    BindGroup, BindGroupLayout, Buffer, BufferDescriptor, BufferUsages, DepthBiasState, Operations,
    RenderPassDescriptor, ShaderStages, TextureUsages, TextureView, TextureViewDescriptor,
    TextureViewDimension,
};

use super::ObjectManager;
use crate::{
    components::{
        cast_shadow, light_kind, light_params, light_shadow_data, projection_matrix, shadow_pass,
    },
    light::{LightKind, LightParams},
    renderer::{
        mesh_renderer::MeshRenderer, CameraData, CameraRenderer, RenderContext, RendererStore,
        UpdateContext,
    },
    rendergraph::{
        BufferHandle, Dependency, Node, NodeExecutionContext, NodeUpdateContext, TextureHandle,
        UpdateResult,
    },
    shader_library::ShaderLibrary,
    types::{shader::TargetDesc, BindGroupBuilder, BindGroupLayoutBuilder},
    Gpu,
};

#[derive(Debug, Clone, Copy)]
pub struct LightShadowData {
    pub index: u32,
    pub cascade_count: u32,
}

#[repr(C)]
#[derive(bytemuck::Pod, bytemuck::Zeroable, Clone, Copy, Debug)]
pub struct LightShadowCamera {
    viewproj: Mat4,
    view: Mat4,
    proj: Mat4,
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
    shadow_casters: Vec<LightShadowCamera>,
    dynamic_light_camera_buffer: Buffer,
    shadow_camera_buffer: BufferHandle,
    renderers: Vec<MeshRenderer>,
    store: RendererStore,
    max_cascades: usize,
    query: Query<ShadowMapNodeQuery>,
    object_manager: Handle<ObjectManager>,
    shader_library: Arc<ShaderLibrary>,
    main_camera_query: Query<(Component<()>, Component<Mat4>, Component<Mat4>)>,
}

fn shader_factory(desc: ShaderDesc) -> ShaderDesc {
    desc.with_depth_bias(DepthBiasState {
        constant: 2,
        slope_scale: 2.0,
        clamp: 0.0,
    })
}

impl ShadowMapNode {
    pub fn new(
        gpu: &Gpu,
        shadow_maps: TextureHandle,
        light_camera_buffer: BufferHandle,
        max_shadows: usize,
        max_cascades: usize,
        shader_library: Arc<ShaderLibrary>,
        object_manager: Handle<ObjectManager>,
    ) -> Self {
        let layout = BindGroupLayoutBuilder::new("LightCameraBuffer")
            .bind_uniform_buffer(ShaderStages::VERTEX)
            .build(gpu);

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
            shader_library,
            shadow_maps,
            max_cascades,
            layout,
            bind_groups: None,
            shadow_casters: vec![],
            dynamic_light_camera_buffer,
            shadow_camera_buffer: light_camera_buffer,
            renderers: Vec::new(),
            store: RendererStore::new(),
            main_camera_query: Query::new((main_camera(), world_transform(), projection_matrix())),
            query: Query::new(ShadowMapNodeQuery {
                id: entity_ids(),
                world_transform: world_transform(),
                light_kind: light_kind(),
                light_params: light_params(),
                cast_shadow: cast_shadow().with(),
            }),
            shadow_map_views: None,
            object_manager,
        }
    }
}

impl Node for ShadowMapNode {
    fn update(&mut self, ctx: NodeUpdateContext) -> anyhow::Result<UpdateResult> {
        profile_function!();

        let shadow_maps = ctx.get_texture(self.shadow_maps);
        let texel_size = vec2(shadow_maps.width() as f32, shadow_maps.height() as f32).recip();

        let mut to_add = Vec::new();

        self.shadow_casters.clear();

        let Some((_, &main_camera_transform, &main_camera_proj)) =
            self.main_camera_query.borrow(ctx.world).first()
        else {
            tracing::warn!("no main camera");
            return Ok(UpdateResult::Success);
        };

        let inv_proj = main_camera_proj.inverse();
        let camera_inv_viewproj = main_camera_transform * inv_proj;

        fn transform_perspective(inv_viewproj: Mat4, clip: Vec3) -> Vec3 {
            let p = inv_viewproj * clip.extend(1.0);
            p.xyz() / p.w
        }

        let near = -transform_perspective(inv_proj, Vec3::ZERO).z;
        let far = -transform_perspective(inv_proj, Vec3::Z).z;

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

            let light_index = self.shadow_casters.len() as u32;

            let light_forward = light_rot * Vec3::FORWARD;
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
                    let view =
                        Mat4::from_rotation_translation(light_rot, light_camera_pos).inverse();

                    let proj =
                        Mat4::orthographic_rh(-radius, radius, -radius, radius, 0.1, radius * 2.0);

                    self.shadow_casters.push(LightShadowCamera {
                        viewproj: proj * view,
                        view,
                        proj,
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
                let view = Mat4::from_rotation_translation(light_rot, light_pos).inverse();

                const MIN_LUM: f32 = 0.01;
                let max_range = (item.light_params.intensity / MIN_LUM).sqrt();

                let proj =
                    Mat4::perspective_rh(item.light_params.outer_theta * 2.0, 1.0, 0.1, max_range);

                self.shadow_casters.push(LightShadowCamera {
                    viewproj: proj * view,
                    view,
                    proj,
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

        if self.renderers.len() < self.shadow_casters.len() {
            self.renderers
                .extend((self.renderers.len()..self.shadow_casters.len()).map(|_| {
                    MeshRenderer::new(
                        ctx.world,
                        ctx.assets,
                        ctx.gpu,
                        shadow_pass(),
                        self.shader_library.clone(),
                    )
                    .with_shader_factory(shader_factory)
                }));
        }

        if self
            .shadow_map_views
            .as_ref()
            .is_some_and(|v| v.len() != self.shadow_casters.len())
        {
            self.shadow_map_views = None;
            self.bind_groups = None;
        }

        ctx.world.append_all(light_shadow_data(), to_add)?;

        let object_manager = ctx.store.get(&self.object_manager);
        let mut update_ctx = UpdateContext {
            world: ctx.world,
            assets: ctx.assets,
            gpu: ctx.gpu,
            store: &mut self.store,
            layouts: &[&self.layout],
            target_desc: TargetDesc {
                formats: &[],
                depth_format: Some(shadow_maps.format()),
                sample_count: shadow_maps.sample_count(),
            },
            object_manager,
        };

        for renderer in &mut self.renderers[0..self.shadow_casters.len()] {
            renderer.update(&mut update_ctx)?;
        }

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
                bytemuck::cast_slice(&self.shadow_casters),
            );

            for (i, &light) in self.shadow_casters.iter().enumerate() {
                ctx.queue.write_buffer(
                    &self.dynamic_light_camera_buffer,
                    i as u64 * light_shadow_stride,
                    bytemuck::cast_slice(&[light]),
                );
            }
        }
        let bind_groups = self.bind_groups.get_or_insert_with(|| {
            profile_scope!("create_bind_groups");

            (0..self.shadow_casters.len())
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

            (0..self.shadow_casters.len())
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
        let iter = izip!(
            bind_groups,
            shadow_map_views,
            &self.shadow_casters,
            &mut self.renderers
        );

        for (bind_group, view, light_camera, renderer) in iter {
            profile_scope!("cascade_draw");

            let object_manager = ctx.store.get(&self.object_manager);
            let draw_ctx = RenderContext {
                world: ctx.world,
                assets: ctx.assets,
                gpu: ctx.gpu,
                queue: ctx.queue,
                store: &self.store,
                bind_groups: &[bind_group],
                layouts: &[&self.layout],
                target_desc: TargetDesc {
                    formats: &[],
                    depth_format: Some(shadow_maps.format()),
                    sample_count: shadow_maps.sample_count(),
                },
                object_manager,
                camera: CameraData {
                    viewproj: light_camera.viewproj,
                    view: light_camera.view,
                    proj: light_camera.proj,
                    camera_pos: light_camera.view.transpose().transform_point3(Vec3::ZERO),
                    fog_blend: Default::default(),
                    fog_color: Default::default(),
                    fog_density: Default::default(),
                },
            };

            renderer.before_draw(&draw_ctx, ctx.encoder)?;

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

            renderer.draw(&draw_ctx, &mut render_pass)?;
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

use glam::{Mat4, Vec3};
use ivy_core::profiling::profile_function;
use ivy_wgpu::{
    renderer::{CameraRenderer, UpdateContext},
    types::{shader::ShaderDesc, BindGroupBuilder, BindGroupLayoutBuilder, Shader, TypedBuffer},
    Gpu,
};
use wgpu::{
    AddressMode, BufferUsages, FilterMode, ShaderModuleDescriptor, ShaderSource, ShaderStages,
};

pub struct SkyboxRenderer {
    shader: Option<Shader>,
    bind_group: wgpu::BindGroup,
    bind_group_layout: wgpu::BindGroupLayout,
    buffer: TypedBuffer<UniformData>,
}

impl SkyboxRenderer {
    pub fn new(gpu: &Gpu) -> Self {
        let bind_group_layout = BindGroupLayoutBuilder::new("skybox")
            .bind_uniform_buffer(ShaderStages::FRAGMENT)
            .bind_sampler(ShaderStages::FRAGMENT)
            .build(gpu);

        let buffer = TypedBuffer::new(
            gpu,
            "skybox",
            BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            &[Default::default()],
        );

        let sampler = gpu.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("skybox_sampler"),
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            address_mode_w: AddressMode::ClampToEdge,
            mag_filter: FilterMode::Nearest,
            min_filter: FilterMode::Nearest,
            mipmap_filter: FilterMode::Nearest,
            ..Default::default()
        });

        let bind_group = BindGroupBuilder::new("skybox")
            .bind_buffer(buffer.buffer())
            .bind_sampler(&sampler)
            .build(gpu, &bind_group_layout);

        Self {
            buffer,
            bind_group,
            bind_group_layout,
            shader: None,
        }
    }
}

impl CameraRenderer for SkyboxRenderer {
    fn update(&mut self, ctx: &mut UpdateContext<'_>) -> anyhow::Result<()> {
        profile_function!();

        self.buffer.write(
            &ctx.gpu.queue,
            0,
            &[UniformData {
                inv_proj: ctx.camera.projection.inverse(),
                inv_view: ctx.camera.view.inverse(),
                fog_color: ctx.camera.fog_color,
                fog_blend: ctx.camera.fog_blend,
            }],
        );

        Ok(())
    }

    fn draw<'s>(
        &'s mut self,
        ctx: &ivy_wgpu::renderer::RenderContext<'s>,
        render_pass: &mut wgpu::RenderPass<'s>,
    ) -> anyhow::Result<()> {
        profile_function!();

        let shader = self.shader.get_or_insert_with(|| {
            Shader::new(
                ctx.gpu,
                &ShaderDesc::new(
                    "skybox_shader",
                    &ctx.gpu.device.create_shader_module(ShaderModuleDescriptor {
                        label: Some("skybox"),
                        source: ShaderSource::Wgsl(include_str!("../shaders/skybox.wgsl").into()),
                    }),
                    &ctx.target_desc,
                )
                .with_bind_group_layouts(&[ctx.layouts[0], &self.bind_group_layout]),
            )
        });

        render_pass.set_pipeline(shader.pipeline());
        render_pass.set_bind_group(0, ctx.bind_groups[0], &[]);
        render_pass.set_bind_group(1, &self.bind_group, &[]);

        render_pass.draw(0..3, 0..1);

        Ok(())
    }
}

#[repr(C)]
#[derive(Default, bytemuck::Pod, bytemuck::Zeroable, Clone, Copy, Debug)]
pub struct UniformData {
    inv_proj: Mat4,
    inv_view: Mat4,
    fog_color: Vec3,
    fog_blend: f32,
}

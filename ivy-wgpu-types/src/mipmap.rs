//! Adapted from https://github.com/gfx-rs/wgpu/blob/trunk/examples/src/mipmap/mod.rs
//!
//! MIT License
//!
//! Copyright (c) 2021 The gfx-rs developers
//!
//! Permission is hereby granted, free of charge, to any person obtaining a copy
//! of this software and associated documentation files (the "Software"), to deal
//! in the Software without restriction, including without limitation the rights
//! to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
//! copies of the Software, and to permit persons to whom the Software is
//! furnished to do so, subject to the following conditions:
//!
//! The above copyright notice and this permission notice shall be included in all
//! copies or substantial portions of the Software.
//!
//! THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
//! IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
//! FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
//! AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
//! LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
//! OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
//! SOFTWARE.

use std::borrow::Cow;

use wgpu::{CommandEncoder, Texture};

use crate::Gpu;

struct MipMapGenerator {}

impl MipMapGenerator {
    fn generate_mipmaps(
        encoder: &mut wgpu::CommandEncoder,
        device: &wgpu::Device,
        texture: &wgpu::Texture,
        mip_count: u32,
        base_array_layer: u32,
    ) {
        let format = texture.format();
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("../shaders/blit.wgsl"))),
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("blit"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                compilation_options: Default::default(),
                targets: &[Some(format.into())],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let bind_group_layout = pipeline.get_bind_group_layout(0);

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("mip"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let views = (0..mip_count)
            .map(|mip| {
                texture.create_view(&wgpu::TextureViewDescriptor {
                    label: Some("mip"),
                    format: None,
                    dimension: Some(wgpu::TextureViewDimension::D2),
                    aspect: wgpu::TextureAspect::All,

                    base_mip_level: mip,
                    mip_level_count: Some(1),
                    base_array_layer,
                    array_layer_count: None,
                })
            })
            .collect::<Vec<_>>();

        for target_mip in 1..mip_count as usize {
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&views[target_mip - 1]),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    },
                ],
                label: None,
            });

            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &views[target_mip],
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            rpass.set_pipeline(&pipeline);
            rpass.set_bind_group(0, &bind_group, &[]);
            rpass.draw(0..3, 0..1);
        }
    }
}

pub fn generate_mipmaps(
    gpu: &Gpu,
    encoder: &mut CommandEncoder,
    texture: &Texture,
    mip_count: u32,
    base_array_layer: u32,
) {
    puffin::profile_function!();

    MipMapGenerator::generate_mipmaps(encoder, &gpu.device, texture, mip_count, base_array_layer);
}

#[cfg(test)]
mod test {
    use ivy_assets::AssetCache;
    use wgpu::{TextureFormat, TextureUsages};

    use crate::{
        mipmap::generate_mipmaps,
        texture::{read_texture, texture_from_image, TextureFromImageDesc},
        Gpu,
    };

    #[test]
    fn load_mips() {
        tracing_subscriber::fmt::init();
        futures::executor::block_on(async {
            tracing::info!("loading image");
            let image = image::open("../assets/textures/statue.jpg").unwrap();

            let gpu = Gpu::headless().await;

            let assets = AssetCache::new();
            assets.register_service(gpu.clone());

            tracing::info!("creating image");
            let texture = texture_from_image(
                &gpu,
                &image,
                TextureFromImageDesc {
                    label: "test_image".into(),
                    format: TextureFormat::Rgba8UnormSrgb,
                    mip_level_count: None,
                    usage: TextureUsages::COPY_SRC
                        | TextureUsages::TEXTURE_BINDING
                        | TextureUsages::RENDER_ATTACHMENT,
                    generate_mipmaps: true,
                },
            )
            .unwrap();

            let mut encoder = gpu.device.create_command_encoder(&Default::default());
            generate_mipmaps(&gpu, &mut encoder, &texture, texture.mip_level_count(), 0);
            gpu.queue.submit([encoder.finish()]);

            tracing::info!("reading back texture");
            let mip = read_texture(&gpu, &texture, 3, 0, image::ColorType::Rgba8)
                .await
                .unwrap();

            mip.save("../assets/textures/mip_output.png").unwrap();
        });
    }
}

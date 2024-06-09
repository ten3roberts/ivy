use std::borrow::Cow;

use ivy_wgpu_types::Gpu;
use slotmap::{SecondaryMap, SlotMap};
use wgpu::{
    Buffer, BufferDescriptor, BufferUsages, Texture, TextureDescriptor, TextureDimension,
    TextureFormat, TextureUsages,
};

use super::{Dependency, Node};

slotmap::new_key_type! {
    pub struct TextureHandle;
    pub struct BufferHandle;
}

pub struct TextureDesc {
    pub label: Cow<'static, str>,
    pub extent: wgpu::Extent3d,
    pub dimension: TextureDimension,
    pub format: TextureFormat,
    pub mip_level_count: u32,
    pub sample_count: u32,
}

pub struct BufferDesc {
    pub label: Cow<'static, str>,
    pub size: u64,
    /// Extra usage flags for e.g; map read
    pub usage: BufferUsages,
}

pub struct Resources {
    textures: SlotMap<TextureHandle, TextureDesc>,
    texture_data: SecondaryMap<TextureHandle, Texture>,
    buffers: SlotMap<BufferHandle, BufferDesc>,
    buffer_data: SecondaryMap<BufferHandle, Buffer>,
    // buffers: Slab<Buffer>,
}

impl Resources {
    pub fn new() -> Self {
        Self {
            textures: Default::default(),
            texture_data: Default::default(),
            buffers: Default::default(),
            buffer_data: Default::default(),
        }
    }

    pub fn insert_texture(&mut self, texture: TextureDesc) -> TextureHandle {
        self.textures.insert(texture)
    }

    pub fn insert_texture_data(&mut self, key: TextureHandle, data: Texture) {
        self.texture_data.insert(key, data);
    }

    pub fn get_texture_data(&self, key: TextureHandle) -> &Texture {
        self.texture_data.get(key).unwrap()
    }

    pub fn insert_buffer(&mut self, buffer: BufferDesc) -> BufferHandle {
        self.buffers.insert(buffer)
    }

    pub fn insert_buffer_data(&mut self, key: BufferHandle, data: Buffer) {
        self.buffer_data.insert(key, data);
    }

    pub fn get_buffer_data(&self, key: BufferHandle) -> &Buffer {
        self.buffer_data.get(key).unwrap()
    }

    pub fn allocate_textures(&mut self, nodes: &[Box<dyn Node>], gpu: &Gpu) {
        let mut usages = SecondaryMap::default();

        nodes
            .iter()
            .flat_map(|v| v.read_dependencies().iter().chain(v.write_dependencies()))
            .for_each(|v| {
                if let &Dependency::Texture { handle, usage } = v {
                    let current_usage = usages
                        .entry(handle)
                        .unwrap()
                        .or_insert(TextureUsages::empty());

                    *current_usage |= usage;
                }
            });

        for (handle, desc) in &self.textures {
            let Some(&usage) = usages.get(handle) else {
                continue;
            };

            tracing::info!(?handle, ?usage, "creating texture for resource");
            let texture = gpu.device.create_texture(&TextureDescriptor {
                label: desc.label.as_ref().into(),
                size: desc.extent,
                mip_level_count: desc.mip_level_count,
                sample_count: desc.sample_count,
                dimension: desc.dimension,
                format: desc.format,
                usage,
                view_formats: &[],
            });

            self.texture_data.insert(handle, texture);
        }
    }

    pub fn allocate_buffers(&mut self, nodes: &[Box<dyn Node>], gpu: &Gpu) {
        let mut usages = SecondaryMap::default();

        nodes
            .iter()
            .flat_map(|v| v.read_dependencies().iter().chain(v.write_dependencies()))
            .for_each(|v| {
                if let &Dependency::Buffer { handle, usage } = v {
                    let current_usage = usages
                        .entry(handle)
                        .unwrap()
                        .or_insert(BufferUsages::empty());

                    *current_usage |= usage;
                }
            });

        for (handle, desc) in &self.buffers {
            let Some(&usage) = usages.get(handle) else {
                continue;
            };

            tracing::info!(?handle, ?usage, "creating texture for resource");
            let buffer = gpu.device.create_buffer(&BufferDescriptor {
                label: desc.label.as_ref().into(),
                size: desc.size,
                usage: desc.usage | usage,
                mapped_at_creation: false,
            });

            self.buffer_data.insert(handle, buffer);
        }
    }
}

impl Default for Resources {
    fn default() -> Self {
        Self::new()
    }
}

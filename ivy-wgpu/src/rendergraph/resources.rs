use std::{borrow::Cow, collections::HashMap};

use itertools::Itertools;
use ivy_wgpu_types::Gpu;
use slotmap::{SecondaryMap, SlotMap};
use wgpu::{
    Buffer, BufferDescriptor, BufferUsages, Texture, TextureDescriptor, TextureDimension,
    TextureFormat, TextureUsages,
};

use super::{Dependency, Node, ResourceHandle};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct Range {
    start: u32,
    end: u32,
}

impl Range {
    fn new(start: u32, end: u32) -> Self {
        Self { start, end }
    }

    fn overlaps(&self, other: Self) -> bool {
        self.start < other.end && self.end > other.start
    }
}

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

type BucketId = usize;

pub struct Bucket<T> {
    desc: T,
    lifetimes: Vec<Range>,
}

impl<T> Bucket<T> {
    pub fn overlaps(&self, lifetime: Range) -> bool {
        self.lifetimes.iter().any(|v| v.overlaps(lifetime))
    }
}

trait SubResource {
    type Desc: Clone;
    fn is_compatible(desc: &Self::Desc, other: &Self::Desc) -> bool;
    fn create(gpu: &Gpu, desc: Self::Desc) -> Self;
}

pub struct SubResources<Handle: slotmap::Key, Data: SubResource> {
    handles: SlotMap<Handle, Data::Desc>,
    bucket_map: SecondaryMap<Handle, BucketId>,
    buckets: Vec<Bucket<Data::Desc>>,
}

impl<Handle: slotmap::Key, Data: SubResource> SubResources<Handle, Data> {
    fn insert(&mut self, desc: Data::Desc) -> Handle {
        self.handles.insert(desc)
    }

    fn allocate_data(&mut self, gpu: &Gpu, lifetimes: HashMap<ResourceHandle, Range>)
    where
        Handle: Into<ResourceHandle>,
    {
        for (handle, desc) in &self.handles {
            let lifetime = *lifetimes.get(&handle.into()).unwrap();
            // Find suitable bucket
            let bucket_id = self
                .buckets
                .iter_mut()
                .find_position(|v| Data::is_compatible(desc, &v.desc) && !v.overlaps(lifetime));

            if let Some((bucket_id, bucket)) = bucket_id {
                bucket.lifetimes.push(lifetime);
                self.bucket_map.insert(handle, bucket_id);
            } else {
                self.bucket_map.insert(handle, self.buckets.len());
                self.buckets.push(Bucket {
                    desc: desc.clone(),
                    lifetimes: vec![lifetime],
                })
            }
        }
    }
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

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
pub(crate) struct Lifetime {
    start: u32,
    end: u32,
}

impl Lifetime {
    pub(crate) fn new(start: u32, end: u32) -> Self {
        Self { start, end }
    }

    pub(crate) fn overlaps(&self, other: Self) -> bool {
        self.start < other.end && self.end > other.start
    }
}

slotmap::new_key_type! {
    pub struct TextureHandle;
    pub struct BufferHandle;
}

#[derive(Debug)]
pub enum TextureDesc {
    External,
    Managed(ManagedTextureDesc),
}

impl From<ManagedTextureDesc> for TextureDesc {
    fn from(v: ManagedTextureDesc) -> Self {
        Self::Managed(v)
    }
}

impl TextureDesc {
    pub fn managed(texture: ManagedTextureDesc) -> Self {
        Self::Managed(texture)
    }

    pub fn as_managed(&self) -> Option<&ManagedTextureDesc> {
        if let Self::Managed(v) = self {
            Some(v)
        } else {
            None
        }
    }

    pub fn as_managed_mut(&mut self) -> Option<&mut ManagedTextureDesc> {
        if let Self::Managed(v) = self {
            Some(v)
        } else {
            None
        }
    }
}

/// Texture data is managed by the render graph
#[derive(Debug, Clone)]
pub struct ManagedTextureDesc {
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

struct Bucket<T> {
    desc: T,
    lifetimes: Vec<Lifetime>,
}

impl<T> Bucket<T> {
    fn overlaps(&self, lifetime: Lifetime) -> bool {
        self.lifetimes.iter().any(|v| v.overlaps(lifetime))
    }
}

trait SubResource {
    type Desc<'a>: Clone;
    fn is_compatible(desc: &Self::Desc<'_>, other: &Self::Desc<'_>) -> bool;
    fn create(gpu: &Gpu, desc: Self::Desc<'_>) -> Self;
}

impl SubResource for Texture {
    type Desc<'a> = TextureDescriptor<'a>;

    fn is_compatible(desc: &Self::Desc<'_>, other: &Self::Desc<'_>) -> bool {
        desc.size == other.size
            && desc.dimension == other.dimension
            && desc.format == other.format
            && desc.mip_level_count == other.mip_level_count
            && desc.sample_count == other.sample_count
    }

    fn create(gpu: &Gpu, desc: Self::Desc<'_>) -> Self {
        gpu.device.create_texture(&desc)
    }
}

impl SubResource for Buffer {
    type Desc<'a> = BufferDescriptor<'a>;

    fn is_compatible(desc: &Self::Desc<'_>, other: &Self::Desc<'_>) -> bool {
        desc.size == other.size
    }

    fn create(gpu: &Gpu, desc: Self::Desc<'_>) -> Self {
        gpu.device.create_buffer(&desc)
    }
}

struct ResourceAllocator<Handle: slotmap::Key, Data: SubResource> {
    bucket_map: SecondaryMap<Handle, BucketId>,
    bucket_data: Vec<Data>,
}

impl<Handle: slotmap::Key, Data: SubResource> ResourceAllocator<Handle, Data> {
    fn new() -> Self {
        Self {
            bucket_map: Default::default(),
            bucket_data: Default::default(),
        }
    }

    fn get(&self, handle: Handle) -> Option<&Data> {
        Some(&self.bucket_data[*self.bucket_map.get(handle)?])
    }

    fn allocate_resources<'a, I: Iterator<Item = (Handle, Data::Desc<'a>, Lifetime)>>(
        &mut self,
        gpu: &Gpu,
        resources: I,
    ) where
        Handle: Into<ResourceHandle>,
    {
        let mut buckets: Vec<Bucket<Data::Desc<'a>>> = Vec::new();

        self.bucket_map.clear();

        for (handle, desc, lifetime) in resources {
            // Find suitable bucket
            let bucket_id = buckets
                .iter_mut()
                .find_position(|v| Data::is_compatible(&desc, &v.desc) && !v.overlaps(lifetime));

            if let Some((bucket_id, bucket)) = bucket_id {
                bucket.lifetimes.push(lifetime);
                self.bucket_map.insert(handle, bucket_id);
            } else {
                self.bucket_map.insert(handle, buckets.len());
                buckets.push(Bucket {
                    desc: desc.clone(),
                    lifetimes: vec![lifetime],
                })
            }
        }

        tracing::info!("Allocating {} resources", buckets.len());

        self.bucket_data = buckets
            .iter()
            .map(|bucket| Data::create(gpu, bucket.desc.clone()))
            .collect_vec();
    }
}

pub struct Resources {
    pub(crate) dirty: bool,

    textures: SlotMap<TextureHandle, TextureDesc>,
    managed_texture_data: ResourceAllocator<TextureHandle, Texture>,

    buffers: SlotMap<BufferHandle, BufferDesc>,
    buffer_data: ResourceAllocator<BufferHandle, Buffer>,
}

impl Resources {
    pub fn new() -> Self {
        Self {
            dirty: false,
            textures: Default::default(),
            buffers: Default::default(),
            managed_texture_data: ResourceAllocator::new(),
            buffer_data: ResourceAllocator::new(),
        }
    }

    pub fn insert_texture(&mut self, texture: impl Into<TextureDesc>) -> TextureHandle {
        self.dirty = true;
        self.textures.insert(texture.into())
    }

    pub fn get_texture_mut(&mut self, handle: TextureHandle) -> &mut TextureDesc {
        self.dirty = true;
        &mut self.textures[handle]
    }

    pub fn get_texture(&self, handle: TextureHandle) -> &TextureDesc {
        &self.textures[handle]
    }

    pub(super) fn get_texture_data(&self, key: TextureHandle) -> &Texture {
        match self.textures.get(key).unwrap() {
            TextureDesc::External => panic!("Must use external resources"),
            TextureDesc::Managed(_) => self.managed_texture_data.get(key).unwrap(),
        }
    }

    pub fn insert_buffer(&mut self, buffer: BufferDesc) -> BufferHandle {
        self.dirty = true;
        self.buffers.insert(buffer)
    }
    pub fn get_buffer_data(&self, key: BufferHandle) -> &Buffer {
        self.buffer_data.get(key).unwrap()
    }

    pub(crate) fn allocate_textures(
        &mut self,
        nodes: &[Box<dyn Node>],
        gpu: &Gpu,
        lifetimes: &HashMap<ResourceHandle, Lifetime>,
    ) {
        let mut usages = SecondaryMap::default();

        nodes
            .iter()
            .flat_map(|v| {
                v.read_dependencies()
                    .into_iter()
                    .chain(v.write_dependencies())
            })
            .for_each(|v| {
                if let Dependency::Texture { handle, usage } = v {
                    let current_usage = usages
                        .entry(handle)
                        .unwrap()
                        .or_insert(TextureUsages::empty());

                    *current_usage |= usage;
                }
            });

        let iter = self.textures.iter().filter_map(|(handle, desc)| {
            let desc = desc.as_managed()?;

            let Some(&lf) = lifetimes.get(&handle.into()) else {
                panic!("No entry for {:?}", self.textures[handle]);
            };

            let usage = *usages.get(handle)?;

            Some((
                handle,
                TextureDescriptor {
                    label: None,
                    size: desc.extent,
                    mip_level_count: desc.mip_level_count,
                    sample_count: desc.sample_count,
                    dimension: desc.dimension,
                    format: desc.format,
                    usage,
                    view_formats: &[],
                },
                lf,
            ))
        });

        self.managed_texture_data.allocate_resources(gpu, iter);
    }

    pub(crate) fn allocate_buffers(
        &mut self,
        nodes: &[Box<dyn Node>],
        gpu: &Gpu,
        lifetimes: &HashMap<ResourceHandle, Lifetime>,
    ) {
        let mut usages = SecondaryMap::default();

        nodes
            .iter()
            .flat_map(|v| {
                v.read_dependencies()
                    .into_iter()
                    .chain(v.write_dependencies())
            })
            .for_each(|v| {
                if let Dependency::Buffer { handle, usage } = v {
                    let current_usage = usages
                        .entry(handle)
                        .unwrap()
                        .or_insert(BufferUsages::empty());

                    *current_usage |= usage;
                }
            });

        let iter = self.buffers.iter().filter_map(|(handle, desc)| {
            let lf = lifetimes[&handle.into()];
            let usage = *usages.get(handle)?;

            Some((
                handle,
                BufferDescriptor {
                    label: desc.label.as_ref().into(),
                    size: desc.size,
                    usage: desc.usage | usage,
                    mapped_at_creation: false,
                },
                lf,
            ))
        });

        self.buffer_data.allocate_resources(gpu, iter);
    }
}

impl Default for Resources {
    fn default() -> Self {
        Self::new()
    }
}

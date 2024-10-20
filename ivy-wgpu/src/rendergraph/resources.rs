use std::{
    borrow::Cow,
    collections::{BTreeSet, HashMap},
};

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
    pub persistent: bool,
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
    fn is_persistent(desc: &Self::Desc<'_>) -> bool;
    fn create(gpu: &Gpu, desc: Self::Desc<'_>) -> Self;
}

#[derive(Debug, Clone)]
struct AllocatedTextureDescriptor {
    desc: ManagedTextureDesc,
    usage: TextureUsages,
}

impl SubResource for Texture {
    type Desc<'a> = AllocatedTextureDescriptor;

    fn is_compatible(desc: &Self::Desc<'_>, other: &Self::Desc<'_>) -> bool {
        !desc.desc.persistent
            && !other.desc.persistent
            && desc.desc.extent == other.desc.extent
            && desc.desc.dimension == other.desc.dimension
            && desc.desc.format == other.desc.format
            && desc.desc.mip_level_count == other.desc.mip_level_count
            && desc.desc.sample_count == other.desc.sample_count
    }

    fn is_persistent(desc: &Self::Desc<'_>) -> bool {
        desc.desc.persistent
    }

    fn create(gpu: &Gpu, desc: Self::Desc<'_>) -> Self {
        gpu.device.create_texture(&TextureDescriptor {
            label: Some(&desc.desc.label),
            size: desc.desc.extent,
            mip_level_count: desc.desc.mip_level_count,
            sample_count: desc.desc.sample_count,
            dimension: desc.desc.dimension,
            format: desc.desc.format,
            usage: desc.usage,
            view_formats: &[],
        })
    }
}

impl SubResource for Buffer {
    type Desc<'a> = BufferDescriptor<'a>;

    fn is_compatible(desc: &Self::Desc<'_>, other: &Self::Desc<'_>) -> bool {
        desc.size == other.size
    }

    fn is_persistent(_desc: &Self::Desc<'_>) -> bool {
        false
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

    fn allocate_resources<'a, I: Iterator<Item = (Handle, Data::Desc<'a>, Option<Lifetime>)>>(
        &mut self,
        gpu: &Gpu,
        resources: I,
    ) where
        Handle: Into<ResourceHandle>,
    {
        let mut buckets: Vec<(Bucket<Data::Desc<'a>>, Vec<Handle>)> = Vec::new();

        for (handle, desc, lifetime) in resources {
            // Find suitable bucket
            let bucket_id = lifetime.and_then(|lifetime| {
                buckets.iter_mut().find_position(|(v, _)| {
                    Data::is_compatible(&desc, &v.desc) && !v.overlaps(lifetime)
                })
            });

            let lifetime = lifetime.unwrap_or(Lifetime::new(0, u32::MAX));

            if let Some((_, (bucket, handles))) = bucket_id {
                bucket.lifetimes.push(lifetime);
                handles.push(handle);
            } else {
                buckets.push((
                    Bucket {
                        desc: desc.clone(),
                        lifetimes: vec![lifetime],
                    },
                    vec![handle],
                ))
            }
        }

        buckets.sort_by_key(|(v, _)| !Data::is_persistent(&v.desc));

        let persistent_count = if self.bucket_data.is_empty() {
            0
        } else {
            buckets
                .iter()
                .take_while(|(v, _)| Data::is_persistent(&v.desc))
                .count()
        };

        self.bucket_data.drain(persistent_count..);

        // Preserve persistent resources
        for (bucket, handles) in &buckets[persistent_count..] {
            let bucket_id = self.bucket_data.len();

            for handle in handles {
                self.bucket_map.insert(*handle, bucket_id);
            }

            self.bucket_data
                .push(Data::create(gpu, bucket.desc.clone()));
        }
    }
}

pub struct Resources {
    pub(crate) dirty: bool,

    textures: SlotMap<TextureHandle, TextureDesc>,
    managed_texture_data: ResourceAllocator<TextureHandle, Texture>,

    buffers: SlotMap<BufferHandle, BufferDesc>,
    buffer_data: ResourceAllocator<BufferHandle, Buffer>,

    pub(crate) modified_resources: BTreeSet<ResourceHandle>,
}

impl Resources {
    pub fn new() -> Self {
        Self {
            dirty: false,
            textures: Default::default(),
            buffers: Default::default(),
            managed_texture_data: ResourceAllocator::new(),
            buffer_data: ResourceAllocator::new(),
            modified_resources: Default::default(),
        }
    }

    pub fn insert_texture(&mut self, texture: impl Into<TextureDesc>) -> TextureHandle {
        self.dirty = true;
        self.textures.insert(texture.into())
    }

    pub fn get_texture_mut(&mut self, handle: TextureHandle) -> &mut TextureDesc {
        self.dirty = true;
        self.modified_resources.insert(handle.into());
        &mut self.textures[handle]
    }

    pub fn get_texture(&self, handle: TextureHandle) -> &TextureDesc {
        &self.textures[handle]
    }

    pub(super) fn get_texture_data(&self, key: TextureHandle) -> &Texture {
        match self.textures.get(key).unwrap() {
            TextureDesc::External => panic!("Must use external resources"),
            TextureDesc::Managed(_) => self.managed_texture_data.get(key).expect("No such texture"),
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
            tracing::info!("allocating {}", desc.label);

            let lf = lifetimes.get(&handle.into()).copied();

            let usage = *usages.get(handle).unwrap();

            Some((
                handle,
                AllocatedTextureDescriptor {
                    desc: desc.clone(),
                    usage,
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
            let lf = lifetimes.get(&handle.into()).copied();
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

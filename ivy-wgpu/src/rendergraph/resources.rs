use std::{
    borrow::Cow,
    collections::{BTreeSet, HashMap},
    sync::Arc,
};

use itertools::Itertools;
use ivy_wgpu_types::Gpu;
use slotmap::{SecondaryMap, SlotMap};
use wgpu::{
    Buffer, BufferAddress, BufferDescriptor, BufferUsages, Texture, TextureDescriptor,
    TextureDimension, TextureFormat, TextureUsages,
};

use crate::shader_library::ShaderLibrary;

use super::{Dependency, Node, NodeId, ResourceHandle};

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

trait SubResource: std::fmt::Debug {
    type Desc: Clone;
    fn is_compatible(desc: &Self::Desc, other: &Self::Desc) -> bool;
    fn is_persistent(desc: &Self::Desc) -> bool;
    fn create(gpu: &Gpu, desc: Self::Desc) -> Self;
}

#[derive(Debug, Clone)]
struct AllocatedTextureDescriptor {
    desc: ManagedTextureDesc,
    usage: TextureUsages,
}

impl SubResource for Texture {
    type Desc = AllocatedTextureDescriptor;

    fn is_compatible(desc: &Self::Desc, other: &Self::Desc) -> bool {
        let inner = &desc.desc;
        inner.extent == other.desc.extent
            && inner.dimension == other.desc.dimension
            && inner.format == other.desc.format
            && inner.mip_level_count == other.desc.mip_level_count
            && inner.sample_count == other.desc.sample_count
            && desc.usage == other.usage
    }

    fn is_persistent(desc: &Self::Desc) -> bool {
        desc.desc.persistent
    }

    fn create(gpu: &Gpu, desc: Self::Desc) -> Self {
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

#[derive(Clone, Debug)]
pub struct AllocatedBufferDescriptor {
    label: Cow<'static, str>,
    size: BufferAddress,
    usage: BufferUsages,
    mapped_at_creation: bool,
}

impl SubResource for Buffer {
    type Desc = AllocatedBufferDescriptor;

    fn is_compatible(desc: &Self::Desc, other: &Self::Desc) -> bool {
        desc.size == other.size
    }

    fn is_persistent(_desc: &Self::Desc) -> bool {
        false
    }

    fn create(gpu: &Gpu, desc: Self::Desc) -> Self {
        gpu.device.create_buffer(&BufferDescriptor {
            label: Some(desc.label.as_ref()),
            size: desc.size,
            usage: desc.usage,
            mapped_at_creation: desc.mapped_at_creation,
        })
    }
}

struct ResourceAllocator<Handle: slotmap::Key, Data: SubResource> {
    bucket_map: SecondaryMap<Handle, BucketId>,
    allocated_desc: SecondaryMap<Handle, Data::Desc>,
    bucket_data: Vec<Data>,
    persistent_count: usize,
}

impl<Handle: slotmap::Key, Data: SubResource> ResourceAllocator<Handle, Data> {
    fn new() -> Self {
        Self {
            bucket_map: Default::default(),
            bucket_data: Default::default(),
            persistent_count: 0,
            allocated_desc: Default::default(),
        }
    }

    fn get(&self, handle: Handle) -> Option<&Data> {
        Some(&self.bucket_data[*self.bucket_map.get(handle)?])
    }

    fn allocate_resources<'a, I: Iterator<Item = (Handle, Data::Desc, Option<Lifetime>)>>(
        &mut self,
        gpu: &Gpu,
        resources: I,
    ) -> anyhow::Result<()>
    where
        Handle: Into<ResourceHandle>,
    {
        let mut new_buckets: Vec<(Bucket<Data::Desc>, Vec<Handle>)> = Vec::new();
        let mut missing_resources: BTreeSet<_> = self.bucket_map.keys().collect();

        for (handle, desc, lifetime) in resources {
            missing_resources.remove(&handle);

            if Data::is_persistent(&desc) && self.bucket_map.contains_key(handle) {
                anyhow::ensure!(
                    Data::is_compatible(&desc, &self.allocated_desc[handle]),
                    "persistent textures can not change allocation parameters"
                );

                continue;
            }

            // Find suitable bucket
            let suitable_bucket = lifetime.and_then(|lifetime| {
                new_buckets.iter_mut().find(|(v, _)| {
                    !(Data::is_persistent(&desc) || Data::is_persistent(&v.desc))
                        && Data::is_compatible(&desc, &v.desc)
                        && !v.overlaps(lifetime)
                })
            });

            let lifetime = lifetime.unwrap_or(Lifetime::new(0, u32::MAX));

            if let Some((bucket, handles)) = suitable_bucket {
                bucket.lifetimes.push(lifetime);
                handles.push(handle);
            } else {
                new_buckets.push((
                    Bucket {
                        desc: desc.clone(),
                        lifetimes: vec![lifetime],
                    },
                    vec![handle],
                ))
            }
        }

        new_buckets.sort_by_key(|(v, _)| !Data::is_persistent(&v.desc));

        // let persistent_count = if self.bucket_data.is_empty() {
        //     0
        // } else {
        //     buckets
        //         .iter()
        //         .take_while(|(v, _)| Data::is_persistent(&v.desc))
        //         .count()
        // };

        for missing in missing_resources {
            self.bucket_map.remove(missing).unwrap();
            let desc = self.allocated_desc.remove(missing).unwrap();

            if Data::is_persistent(&desc) {
                anyhow::bail!("persistent textures can not be removed")
            }
        }

        let new_persistent_count = new_buckets
            .iter()
            .take_while(|(v, _)| Data::is_persistent(&v.desc))
            .count();

        self.bucket_data.drain(self.persistent_count..);

        // Preserve persistent resources
        for (bucket, handles) in &new_buckets[..] {
            let bucket_id = self.bucket_data.len();

            for &handle in handles {
                self.allocated_desc.insert(handle, bucket.desc.clone());
                self.bucket_map.insert(handle, bucket_id);
            }

            self.bucket_data
                .push(Data::create(gpu, bucket.desc.clone()));
        }

        self.persistent_count += new_persistent_count;

        Ok(())
    }
}

pub struct RenderGraphResources {
    pub(crate) dirty: bool,
    shader_library: Arc<ShaderLibrary>,

    textures: SlotMap<TextureHandle, TextureDesc>,
    managed_texture_data: ResourceAllocator<TextureHandle, Texture>,

    buffers: SlotMap<BufferHandle, BufferDesc>,
    buffer_data: ResourceAllocator<BufferHandle, Buffer>,

    pub(crate) modified_resources: BTreeSet<ResourceHandle>,
}

impl RenderGraphResources {
    pub fn new(shader_library: Arc<ShaderLibrary>) -> Self {
        Self {
            dirty: false,
            textures: Default::default(),
            buffers: Default::default(),
            managed_texture_data: ResourceAllocator::new(),
            buffer_data: ResourceAllocator::new(),
            modified_resources: Default::default(),
            shader_library,
        }
    }

    pub fn insert_texture(&mut self, texture: impl Into<TextureDesc>) -> TextureHandle {
        self.dirty = true;
        self.textures.insert(texture.into())
    }

    pub fn remove_texture(&mut self, texture: TextureHandle) -> Option<TextureDesc> {
        self.dirty = true;
        self.textures.remove(texture)
    }

    pub fn get_texture_mut(&mut self, handle: TextureHandle) -> &mut TextureDesc {
        self.dirty = true;
        self.modified_resources.insert(handle.into());
        &mut self.textures[handle]
    }

    pub fn get_texture(&self, handle: TextureHandle) -> &TextureDesc {
        &self.textures[handle]
    }

    #[track_caller]
    pub(super) fn get_texture_data(&self, key: TextureHandle) -> &Texture {
        match self.textures.get(key).unwrap() {
            TextureDesc::External => panic!("Must use external resources"),
            TextureDesc::Managed(_) => match self.managed_texture_data.get(key) {
                Some(v) => v,
                None => {
                    panic!("No such texture {key:?}");
                }
            },
        }
    }

    pub fn insert_buffer(&mut self, buffer: BufferDesc) -> BufferHandle {
        self.dirty = true;
        self.buffers.insert(buffer)
    }

    pub fn remove_buffer(&mut self, texture: BufferHandle) -> Option<BufferDesc> {
        self.dirty = true;
        self.buffers.remove(texture)
    }

    pub fn get_buffer_data(&self, key: BufferHandle) -> &Buffer {
        self.buffer_data.get(key).unwrap()
    }

    pub(crate) fn allocate_textures(
        &mut self,
        nodes: &SlotMap<NodeId, Box<dyn Node>>,
        gpu: &Gpu,
        lifetimes: &HashMap<ResourceHandle, Lifetime>,
    ) -> anyhow::Result<()> {
        let mut usages = SecondaryMap::default();

        nodes
            .values()
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

            let _span = tracing::info_span!("allocating", ?desc.label, ?handle).entered();

            let lf = lifetimes.get(&handle.into()).copied();

            let Some(&usage) = usages.get(handle) else {
                tracing::warn!("no usages for {}", desc.label);
                return None;
            };

            tracing::info!(?usage);

            Some((
                handle,
                AllocatedTextureDescriptor {
                    desc: desc.clone(),
                    usage,
                },
                lf,
            ))
        });

        self.managed_texture_data.allocate_resources(gpu, iter)
    }

    pub(crate) fn allocate_buffers(
        &mut self,
        nodes: &SlotMap<NodeId, Box<dyn Node>>,
        gpu: &Gpu,
        lifetimes: &HashMap<ResourceHandle, Lifetime>,
    ) -> anyhow::Result<()> {
        let mut usages = SecondaryMap::default();

        nodes
            .values()
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
                AllocatedBufferDescriptor {
                    label: desc.label.clone(),
                    size: desc.size,
                    usage: desc.usage | usage,
                    mapped_at_creation: false,
                },
                lf,
            ))
        });

        self.buffer_data.allocate_resources(gpu, iter)
    }

    pub fn shader_library(&self) -> &Arc<ShaderLibrary> {
        &self.shader_library
    }
}

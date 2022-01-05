use std::{
    any::{type_name, TypeId},
    borrow::Cow,
    collections::HashMap,
};

use ivy_resources::{Handle, HandleUntyped};
use ivy_vulkan::ShaderPass;

use crate::{Error, Result};

ivy_vulkan::new_shaderpass! {
    pub struct GeometryPass;
    pub struct SkinnedPass;
    pub struct ImagePass;
    pub struct TextPass;
    pub struct GizmoPass;
    pub struct PostProcessingPass;
}

/// Stores handles to pipelines created by the presets.
/// Not all pipelines may be present depending on the preset.
/// The same struct is used for all presets to minimize code changes when
/// changing preset.
// #[records::record]
pub struct PipelineStore {
    by_type: HashMap<TypeId, Vec<NamedPipeline>>,
}

#[records::record]
struct NamedPipeline {
    handle: HandleUntyped,
    name: Cow<'static, str>,
}

impl PipelineStore {
    pub fn new() -> Self {
        Self {
            by_type: HashMap::new(),
        }
    }

    pub fn insert<T: ShaderPass, S: Into<Cow<'static, str>>>(
        &mut self,
        pipeline: Handle<T>,
        name: S,
    ) {
        let pipeline = pipeline.into_untyped();
        self.by_type
            .entry(TypeId::of::<T>())
            .or_default()
            .push(NamedPipeline::new(pipeline, name.into()))
    }

    /// Retrieves the first pipeline of type `T`
    pub fn default<T: 'static>(&self) -> Result<Handle<T>> {
        let pipeline = &self
            .by_type
            .get(&TypeId::of::<T>())
            .ok_or_else(|| Error::MissingPipeline(type_name::<T>()))?[0];

        Ok(Handle::from_untyped(pipeline.handle))
    }

    /// Retrieves the pipeline of type `T` by name
    pub fn by_name<T: 'static>(&self, name: &str) -> Result<Handle<T>> {
        let pipeline = self
            .by_type
            .get(&TypeId::of::<T>())
            .ok_or_else(|| Error::MissingPipeline(type_name::<T>()))?
            .iter()
            .find(|val| val.name == name)
            .ok_or_else(|| Error::MissingPipelineName(name.to_owned(), type_name::<T>()))?;

        Ok(Handle::from_untyped(pipeline.handle))
    }
}

use std::{borrow::Cow, collections::HashMap};

use anyhow::Context;
use ivy_wgpu_types::Gpu;
use naga_oil::compose::{Composer, ShaderDefValue};
use parking_lot::Mutex;
use wgpu::{Device, ShaderModule, ShaderModuleDescriptor, ShaderSource};

use crate::shader::ShaderPassDesc;

pub struct ModuleDesc<'a> {
    pub path: &'a str,
    pub source: &'a str,
}

pub struct ShaderModuleDesc<'a> {
    pub label: &'a str,
    pub path: &'a str,
    pub source: &'a str,
    pub shader_defs: HashMap<String, ShaderDefValue>,
}

impl<'a> From<&'a ShaderPassDesc> for ShaderModuleDesc<'a> {
    fn from(value: &'a ShaderPassDesc) -> Self {
        Self {
            path: &value.path,
            source: &value.source,
            shader_defs: Default::default(),
            label: &value.label,
        }
    }
}

pub struct ShaderLibrary {
    composer: Mutex<Composer>,
}

impl ShaderLibrary {
    pub fn new() -> Self {
        Self {
            composer: Mutex::new(Composer::default()),
        }
    }

    pub fn with_module(mut self, module: ModuleDesc) -> Self {
        match self.composer.get_mut().add_composable_module(
            naga_oil::compose::ComposableModuleDescriptor {
                source: module.source,
                file_path: module.path,
                ..Default::default()
            },
        ) {
            Ok(_) => {
                tracing::info!("Added module");
            }
            Err(err) => {
                tracing::error!("Failed to add module: {err:?}");
            }
        }

        self
    }

    pub fn process(
        &self,
        gpu: &Gpu,
        module_desc: ShaderModuleDesc,
    ) -> anyhow::Result<ShaderModule> {
        let module = self
            .composer
            .lock()
            .make_naga_module(naga_oil::compose::NagaModuleDescriptor {
                source: module_desc.source,
                file_path: module_desc.path,
                shader_type: naga_oil::compose::ShaderType::Wgsl,
                shader_defs: module_desc.shader_defs,
                ..Default::default()
            })
            .with_context(|| {
                anyhow::anyhow!("Failed to process shader module {:?}", module_desc.path)
            })?;

        Ok(gpu.device.create_shader_module(ShaderModuleDescriptor {
            source: ShaderSource::Naga(Cow::Owned(module)),
            label: Some(module_desc.label),
        }))
    }
}

impl Default for ShaderLibrary {
    fn default() -> Self {
        Self::new()
    }
}

use std::{borrow::Cow, collections::HashMap};

use anyhow::Context;
use ivy_wgpu_types::Gpu;
use naga_oil::compose::{Composer, ShaderDefValue};
use parking_lot::Mutex;
use wgpu::{ShaderModule, ShaderModuleDescriptor, ShaderSource};

use crate::shader::{ShaderPass, ShaderValue};

pub struct ShaderModuleDesc<'a> {
    pub path: &'a str,
    pub source: &'a str,
    pub shader_defs: HashMap<String, ShaderDefValue>,
}

impl<'a> From<&'a ShaderPass> for ShaderModuleDesc<'a> {
    fn from(value: &'a ShaderPass) -> Self {
        Self {
            path: &value.path,
            source: &value.source,
            shader_defs: value
                .shader_defs
                .iter()
                .map(|(k, v)| (k.clone(), (*v).into()))
                .collect(),
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

    pub fn with_module(mut self, module: ShaderModuleDesc) -> Self {
        match self.composer.get_mut().add_composable_module(
            naga_oil::compose::ComposableModuleDescriptor {
                source: module.source,
                file_path: module.path,
                shader_defs: module.shader_defs,
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

    pub fn process(&self, gpu: &Gpu, module: ShaderModuleDesc) -> anyhow::Result<ShaderModule> {
        let naga_module = self
            .composer
            .lock()
            .make_naga_module(naga_oil::compose::NagaModuleDescriptor {
                source: module.source,
                file_path: module.path,
                shader_type: naga_oil::compose::ShaderType::Wgsl,
                shader_defs: module.shader_defs,
                ..Default::default()
            })
            .with_context(|| {
                anyhow::anyhow!("Failed to process shader module {:?}", module.path)
            })?;

        Ok(gpu.device.create_shader_module(ShaderModuleDescriptor {
            source: ShaderSource::Naga(Cow::Owned(naga_module)),
            label: Some(module.path),
        }))
    }
}

impl Default for ShaderLibrary {
    fn default() -> Self {
        Self::new()
    }
}

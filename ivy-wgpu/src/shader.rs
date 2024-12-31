use std::{borrow::Cow, collections::BTreeMap};

use wgpu::Face;

/// Represents a shader
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ShaderPass {
    pub path: String,
    pub label: Cow<'static, str>,
    pub source: Cow<'static, str>,
    pub cull_mode: Option<Face>,
    pub shader_defs: BTreeMap<String, ShaderValue>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ShaderValue {
    Bool(bool),
    Int(i32),
    UInt(u32),
}

impl From<ShaderValue> for naga_oil::compose::ShaderDefValue {
    fn from(value: ShaderValue) -> Self {
        match value {
            ShaderValue::Bool(v) => naga_oil::compose::ShaderDefValue::Bool(v),
            ShaderValue::Int(v) => naga_oil::compose::ShaderDefValue::Int(v),
            ShaderValue::UInt(v) => naga_oil::compose::ShaderDefValue::UInt(v),
        }
    }
}

impl ShaderPass {
    pub fn new(
        path: impl Into<String>,
        label: impl Into<Cow<'static, str>>,
        source: impl Into<Cow<'static, str>>,
        shader_defs: impl IntoIterator<Item = (String, ShaderValue)>,
    ) -> Self {
        Self {
            path: path.into(),
            label: label.into(),
            source: source.into(),
            cull_mode: None,
            shader_defs: shader_defs.into_iter().collect(),
        }
    }

    /// Set the cull mode
    pub fn with_cull_mode(mut self, cull_mode: Face) -> Self {
        self.cull_mode = Some(cull_mode);
        self
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn label(&self) -> &str {
        &self.label
    }
}

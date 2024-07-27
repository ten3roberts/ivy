use std::borrow::Cow;

use wgpu::Face;

/// Describes a shader
pub struct ShaderPassDesc {
    pub label: String,
    pub source: Cow<'static, str>,
    pub cull_mode: Option<Face>,
}

impl ShaderPassDesc {
    pub fn new(label: impl Into<String>, source: impl Into<Cow<'static, str>>) -> Self {
        Self {
            label: label.into(),
            source: source.into(),
            cull_mode: None,
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

use std::borrow::Cow;

use wgpu::Face;

/// Represents a shader
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ShaderPass {
    pub path: String,
    pub label: String,
    pub source: Cow<'static, str>,
    pub cull_mode: Option<Face>,
}

impl ShaderPass {
    pub fn new(
        path: impl Into<String>,
        label: impl Into<String>,
        source: impl Into<Cow<'static, str>>,
    ) -> Self {
        Self {
            path: path.into(),
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

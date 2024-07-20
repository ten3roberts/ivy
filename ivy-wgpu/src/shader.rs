use std::borrow::Cow;

/// Describes a shader
pub struct ShaderPassDesc {
    pub label: String,
    pub source: Cow<'static, str>,
}

impl ShaderPassDesc {
    pub fn new(label: impl Into<String>, source: impl Into<Cow<'static, str>>) -> Self {
        Self {
            label: label.into(),
            source: source.into(),
        }
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn label(&self) -> &str {
        &self.label
    }
}

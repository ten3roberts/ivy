/// Describes a shader
pub struct ShaderDesc {
    label: String,
    source: String,
}

impl ShaderDesc {
    pub fn new(label: impl Into<String>, source: impl Into<String>) -> Self {
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

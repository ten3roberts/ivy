use std::convert::Infallible;

use ivy_assets::{Asset, AssetCache, AssetDesc};
use wgpu::{Face, PolygonMode};

use crate::shader::{ShaderPass, ShaderValue};

/// Loads the default PBR shader
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PbrShaderDesc {
    pub skinned: bool,
    pub lit: bool,
    pub polygon_mode: PolygonMode,
}

impl AssetDesc for PbrShaderDesc {
    type Output = ShaderPass;
    type Error = Infallible;

    fn create(&self, assets: &ivy_assets::AssetCache) -> Result<Asset<ShaderPass>, Self::Error> {
        Ok(assets.insert(ShaderPass {
            label: "pbr_shader".into(),
            path: "pbr.wgsl".into(),
            source: include_str!("../../assets/shaders/pbr.wgsl").into(),
            cull_mode: Some(Face::Back),
            shader_defs: [
                self.skinned
                    .then(|| ("SKINNED".into(), ShaderValue::Bool(true))),
                self.lit.then(|| ("LIT".into(), ShaderValue::Bool(true))),
            ]
            .into_iter()
            .flatten()
            .collect(),
            polygon_mode: self.polygon_mode,
        }))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ShadowShaderDesc {
    pub skinned: bool,
}

impl AssetDesc for ShadowShaderDesc {
    type Output = ShaderPass;
    type Error = Infallible;

    fn create(&self, assets: &ivy_assets::AssetCache) -> Result<Asset<ShaderPass>, Self::Error> {
        Ok(assets.insert(ShaderPass {
            label: "shadow_shader".into(),
            path: "../../assets/shaders/shadow.wgsl".into(),
            source: include_str!("../../assets/shaders/shadow.wgsl").into(),
            cull_mode: Some(Face::Back),
            shader_defs: [self
                .skinned
                .then(|| ("SKINNED".into(), ShaderValue::Bool(true)))]
            .into_iter()
            .flatten()
            .collect(),
            polygon_mode: PolygonMode::Fill,
        }))
    }
}

/// Emissive textured pbr material
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PbrEmissiveShaderDesc {
    pub skinned: bool,
    pub lit: bool,
}

impl AssetDesc for PbrEmissiveShaderDesc {
    type Output = ShaderPass;
    type Error = Infallible;

    fn create(&self, assets: &AssetCache) -> Result<Asset<ShaderPass>, Self::Error> {
        Ok(assets.insert(ShaderPass {
            label: "pbr_emissive_shader".into(),
            path: "pbr_emissive.wgsl".into(),
            source: include_str!("../../assets/shaders/pbr_emissive.wgsl").into(),
            cull_mode: Some(Face::Back),
            shader_defs: [
                self.skinned
                    .then(|| ("SKINNED".into(), ShaderValue::Bool(true))),
                self.lit.then(|| ("LIT".into(), ShaderValue::Bool(true))),
            ]
            .into_iter()
            .flatten()
            .collect(),
            polygon_mode: PolygonMode::Fill,
        }))
    }
}

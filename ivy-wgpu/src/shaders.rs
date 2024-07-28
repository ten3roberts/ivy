use std::convert::Infallible;

use ivy_assets::{Asset, AssetDesc};
use wgpu::Face;

use crate::shader::ShaderPassDesc;

/// Loads the default PBR shader
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PbrShaderDesc;

impl AssetDesc<ShaderPassDesc> for PbrShaderDesc {
    type Error = Infallible;

    fn load(&self, assets: &ivy_assets::AssetCache) -> Result<Asset<ShaderPassDesc>, Self::Error> {
        Ok(assets.insert(ShaderPassDesc {
            label: "pbr_shader".into(),
            path: "../../assets/shader/pbr.wgsl".into(),
            source: include_str!("../../assets/shaders/pbr.wgsl").into(),
            cull_mode: Some(Face::Back),
        }))
    }
}

/// Loads the default skinned PBR shader
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SkinnedPbrShaderDesc;

impl AssetDesc<ShaderPassDesc> for SkinnedPbrShaderDesc {
    type Error = Infallible;

    fn load(&self, assets: &ivy_assets::AssetCache) -> Result<Asset<ShaderPassDesc>, Self::Error> {
        Ok(assets.insert(ShaderPassDesc {
            label: "skinned_pbr_shader".into(),
            path: "../../assets/shaders/skinned_pbr.wgsl".into(),
            source: include_str!("../../assets/shaders/skinned_pbr.wgsl").into(),
            cull_mode: None,
        }))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ShadowShaderDesc;

impl AssetDesc<ShaderPassDesc> for ShadowShaderDesc {
    type Error = Infallible;

    fn load(&self, assets: &ivy_assets::AssetCache) -> Result<Asset<ShaderPassDesc>, Self::Error> {
        Ok(assets.insert(ShaderPassDesc {
            label: "shadow_shader".into(),
            path: "../../assets/shaders/shadow.wgsl".into(),
            source: include_str!("../../assets/shaders/shadow.wgsl").into(),
            cull_mode: Some(Face::Front),
        }))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SkinnedShadowShaderDesc;

impl AssetDesc<ShaderPassDesc> for SkinnedShadowShaderDesc {
    type Error = Infallible;

    fn load(&self, assets: &ivy_assets::AssetCache) -> Result<Asset<ShaderPassDesc>, Self::Error> {
        Ok(assets.insert(ShaderPassDesc {
            label: "skinned_shadow_shader".into(),
            path: "../../assets/shaders/skinned_shadow.wgsl".into(),
            source: include_str!("../../assets/shaders/skinned_shadow.wgsl").into(),
            cull_mode: Some(Face::Front),
        }))
    }
}

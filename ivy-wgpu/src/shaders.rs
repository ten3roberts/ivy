use std::convert::Infallible;

use ivy_assets::{Asset, AssetDesc};
use wgpu::Face;

use crate::shader::ShaderPass;

/// Loads the default PBR shader
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PbrShaderDesc;

impl AssetDesc<ShaderPass> for PbrShaderDesc {
    type Error = Infallible;

    fn create(&self, assets: &ivy_assets::AssetCache) -> Result<Asset<ShaderPass>, Self::Error> {
        Ok(assets.insert(ShaderPass {
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

impl AssetDesc<ShaderPass> for SkinnedPbrShaderDesc {
    type Error = Infallible;

    fn create(&self, assets: &ivy_assets::AssetCache) -> Result<Asset<ShaderPass>, Self::Error> {
        Ok(assets.insert(ShaderPass {
            label: "skinned_pbr_shader".into(),
            path: "../../assets/shaders/skinned_pbr.wgsl".into(),
            source: include_str!("../../assets/shaders/skinned_pbr.wgsl").into(),
            cull_mode: None,
        }))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ShadowShaderDesc;

impl AssetDesc<ShaderPass> for ShadowShaderDesc {
    type Error = Infallible;

    fn create(&self, assets: &ivy_assets::AssetCache) -> Result<Asset<ShaderPass>, Self::Error> {
        Ok(assets.insert(ShaderPass {
            label: "shadow_shader".into(),
            path: "../../assets/shaders/shadow.wgsl".into(),
            source: include_str!("../../assets/shaders/shadow.wgsl").into(),
            cull_mode: Some(Face::Front),
        }))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SkinnedShadowShaderDesc;

impl AssetDesc<ShaderPass> for SkinnedShadowShaderDesc {
    type Error = Infallible;

    fn create(&self, assets: &ivy_assets::AssetCache) -> Result<Asset<ShaderPass>, Self::Error> {
        Ok(assets.insert(ShaderPass {
            label: "skinned_shadow_shader".into(),
            path: "../../assets/shaders/skinned_shadow.wgsl".into(),
            source: include_str!("../../assets/shaders/skinned_shadow.wgsl").into(),
            cull_mode: Some(Face::Front),
        }))
    }
}

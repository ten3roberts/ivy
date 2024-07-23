use std::convert::Infallible;

use ivy_assets::{Asset, AssetDesc};

use crate::shader::ShaderPassDesc;

/// Loads the default PBR shader
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PbrShaderDesc;

impl AssetDesc<ShaderPassDesc> for PbrShaderDesc {
    type Error = Infallible;

    fn load(&self, assets: &ivy_assets::AssetCache) -> Result<Asset<ShaderPassDesc>, Self::Error> {
        let source = include_str!("../../assets/shaders/pbr.wgsl").into();

        Ok(assets.insert(ShaderPassDesc {
            label: "pbr_shader".into(),
            source,
        }))
    }
}

/// Loads the default skinned PBR shader
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SkinnedPbrShaderDesc;

impl AssetDesc<ShaderPassDesc> for SkinnedPbrShaderDesc {
    type Error = Infallible;

    fn load(&self, assets: &ivy_assets::AssetCache) -> Result<Asset<ShaderPassDesc>, Self::Error> {
        let source = include_str!("../../assets/shaders/skinned_pbr.wgsl").into();

        Ok(assets.insert(ShaderPassDesc {
            label: "skinned_pbr_shader".into(),
            source,
        }))
    }
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ShadowShaderDesc;

impl AssetDesc<ShaderPassDesc> for ShadowShaderDesc {
    type Error = Infallible;

    fn load(&self, assets: &ivy_assets::AssetCache) -> Result<Asset<ShaderPassDesc>, Self::Error> {
        let source = include_str!("../../assets/shaders/shadow.wgsl").into();

        Ok(assets.insert(ShaderPassDesc {
            label: "shadow_shader".into(),
            source,
        }))
    }
}

use std::convert::Infallible;

use ivy_assets::{Asset, AssetDesc};

use crate::shader::ShaderDesc;

/// Loads the default PBR shader
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PbrShaderDesc;

impl AssetDesc<ShaderDesc> for PbrShaderDesc {
    type Error = Infallible;

    fn load(&self, assets: &ivy_assets::AssetCache) -> Result<Asset<ShaderDesc>, Self::Error> {
        let source = include_str!("../../assets/shaders/pbr.wgsl").into();

        Ok(assets.insert(ShaderDesc {
            label: "pbr_shader".into(),
            source,
        }))
    }
}

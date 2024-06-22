use std::convert::Infallible;

use ivy_assets::{Asset, AssetKey};

use crate::shader::ShaderDesc;

/// Loads the default PBR shader
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PbrShaderKey;

impl AssetKey<ShaderDesc> for PbrShaderKey {
    type Error = Infallible;

    fn load(&self, assets: &ivy_assets::AssetCache) -> Result<Asset<ShaderDesc>, Self::Error> {
        let source = include_str!("../../assets/shaders/pbr.wgsl").into();

        Ok(assets.insert(ShaderDesc {
            label: "pbr_shader".into(),
            source,
        }))
    }
}

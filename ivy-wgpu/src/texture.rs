use ivy_assets::{Asset, AssetCache, AssetDesc, AssetExt};
use ivy_core::profiling::profile_function;
use ivy_graphics::texture::TextureData;
use ivy_wgpu_types::texture::{texture_from_image, TextureFromImageDesc};
use wgpu::{Texture, TextureFormat};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct TextureWithFormatDesc {
    texture: TextureData,
    format: TextureFormat,
}

impl TextureWithFormatDesc {
    pub(crate) fn new(texture: TextureData, format: TextureFormat) -> Self {
        Self { texture, format }
    }
}

impl AssetDesc for TextureWithFormatDesc {
    type Output = Texture;
    type Error = anyhow::Error;

    fn create(&self, assets: &AssetCache) -> Result<Asset<Texture>, Self::Error> {
        profile_function!("TextureDesc::load");
        let gpu = assets.service();

        let image = self.texture.load(assets)?;

        let texture = texture_from_image(
            &gpu,
            &image,
            TextureFromImageDesc {
                label: "content".into(),
                format: self.format,
                ..Default::default()
            },
        )?;

        Ok(assets.insert(texture))
    }
}

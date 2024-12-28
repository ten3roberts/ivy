use ivy_assets::{Asset, AssetCache, AssetDesc};
use ivy_core::profiling::profile_function;
use ivy_graphics::texture::TextureDesc;
use ivy_wgpu_types::texture::{texture_from_image, TextureFromColor, TextureFromImageDesc};
use wgpu::{Texture, TextureFormat};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct TextureAndKindDesc {
    texture: TextureDesc,
    format: TextureFormat,
}

impl TextureAndKindDesc {
    pub(crate) fn new(texture: TextureDesc, format: TextureFormat) -> Self {
        Self { texture, format }
    }
}

impl AssetDesc<Texture> for TextureAndKindDesc {
    type Error = anyhow::Error;

    fn create(&self, assets: &AssetCache) -> Result<Asset<Texture>, Self::Error> {
        profile_function!("TextureDesc::load");
        let gpu = assets.service();

        match &self.texture {
            TextureDesc::Content(image) => Ok(assets.insert(
                texture_from_image(
                    &gpu,
                    image,
                    TextureFromImageDesc {
                        label: "content".into(),
                        format: self.format,
                        ..Default::default()
                    },
                )
                .unwrap(),
            )),
            TextureDesc::Color(v) => Ok(assets.load(&TextureFromColor {
                color: v.0,
                format: self.format,
            })),
            v => {
                let image = assets.try_load(v)?;

                Ok(assets.insert(
                    texture_from_image(
                        &gpu,
                        &image,
                        TextureFromImageDesc {
                            label: v.label().to_owned().into(),
                            format: self.format,
                            ..Default::default()
                        },
                    )
                    .unwrap(),
                ))
            }
        }
    }
}

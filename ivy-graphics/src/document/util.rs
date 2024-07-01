use std::path::Path;

use gltf::{buffer, image};
use ivy_assets::{Asset, AssetCache};
use ivy_vulkan::{
    context::{VulkanContextService},
    Texture, TextureFromMemory,
};

use crate::{Error, Result, Scheme};

/// Import the buffer data referenced by a glTF document.
pub fn import_buffer_data(
    document: &gltf::Document,
    mut blob: Option<Vec<u8>>,
    base: &Path,
) -> Result<Vec<buffer::Data>> {
    document
        .buffers()
        .map(|buffer| {
            let mut data = match buffer.source() {
                buffer::Source::Uri(uri) => Scheme::read(base, uri),
                buffer::Source::Bin => blob.take().ok_or(Error::GltfImport(
                    gltf::Error::MissingBlob,
                    Some(base.to_owned()),
                )),
            }?;
            if data.len() < buffer.length() {
                return Err(Error::GltfImport(
                    gltf::Error::BufferLength {
                        buffer: buffer.index(),
                        expected: buffer.length(),
                        actual: data.len(),
                    },
                    Some(base.to_owned()),
                ));
            }
            while data.len() % 4 != 0 {
                data.push(0);
            }
            Ok(buffer::Data(data))
        })
        .collect()
}

/// Import the image data referenced by a glTF document.
pub fn import_image_data(
    assets: &AssetCache,
    document: &gltf::Document,
    base: &Path,
    buffer_data: &[buffer::Data],
) -> Result<Vec<Asset<Texture>>> {
    document
        .textures()
        .map(|tex| -> Result<Asset<Texture>> {
            match tex.source().source() {
                image::Source::Uri { uri, mime_type: _ } => {
                    let data = Scheme::read(base, uri)?;

                    let texture = assets.load(&TextureFromMemory(data));
                    Ok(texture)
                }
                image::Source::View { view, mime_type: _ } => {
                    let parent_buffer_data = &buffer_data[view.buffer().index()].0;
                    let begin = view.offset();
                    let end = begin + view.length();
                    let encoded_image = &parent_buffer_data[begin..end];
                    let texture = Texture::from_memory(
                        assets.service::<VulkanContextService>().context(),
                        encoded_image,
                    )?;

                    Ok(assets.insert(texture))
                }
            }
        })
        .collect::<Result<Vec<_>>>()
}

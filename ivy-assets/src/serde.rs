use std::path::Path;

use crate::{fs::AsyncAssetFromPath, Asset, AssetCache};

pub trait AsyncAssetFromJson: ::serde::de::DeserializeOwned {}

impl<V> AsyncAssetFromPath for V
where
    V: 'static + AsyncAssetFromJson + Send + Sync,
{
    type Error = anyhow::Error;

    async fn load_from_path(path: &Path, assets: &AssetCache) -> Result<Asset<V>, Self::Error> {
        let content = assets.try_load_async::<_, Vec<u8>>(path).await?;
        let data = serde_json::from_slice(&content[..])?;

        Ok(assets.insert(data))
    }
}

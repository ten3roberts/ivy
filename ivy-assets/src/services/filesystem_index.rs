use std::{
    collections::BTreeMap,
    mem,
    path::PathBuf,
    sync::{Arc, OnceLock},
};

use async_std::stream::StreamExt;
use futures::future::BoxFuture;
use parking_lot::Mutex;
use violet::core::to_owned;

use crate::{
    meta::AssetPayloadUntyped,
    service::{FileSystemMapService, Service},
    AssetCache, AssetPath,
};

/// Indexes all assets in the configured root
pub struct FileSystemIndexService {
    index: OnceLock<FileSystemIndex>,
}

impl FileSystemIndexService {
    pub fn new() -> Self {
        Self {
            index: OnceLock::new(),
        }
    }

    pub fn get_index(&self) -> Option<&FileSystemIndex> {
        self.index.get()
    }
}

impl Service for FileSystemIndexService {
    fn register(&self, assets: &AssetCache) {
        let fs = assets.service::<FileSystemMapService>();

        to_owned!(assets, root = fs.root());
        async_std::task::spawn(async move {
            let index = Arc::new(Mutex::new(Default::default()));

            let index = index_dir(&root, &assets, index, &root).await;

            let index = mem::take(&mut *index.lock());

            tracing::info!("Indexed {} assets", index.assets.len());
            assets.service::<FileSystemIndexService>().index.set(index)
        });
    }
}

fn index_dir<'a>(
    root: &'a PathBuf,
    assets: &'a AssetCache,
    result: Arc<Mutex<FileSystemIndex>>,
    dir: &'a PathBuf,
) -> BoxFuture<'a, Arc<Mutex<FileSystemIndex>>> {
    Box::pin(async move {
        let mut entries = match async_std::fs::read_dir(dir).await {
            Ok(entries) => entries,
            Err(_) => return result,
        };

        let mut spawned_tasks = Vec::new();

        while let Some(entry) = entries.next().await {
            let entry = match entry {
                Ok(entry) => entry,
                Err(_) => continue,
            };

            let path = entry.path();
            if path.is_dir().await {
                index_dir(root, assets, result.clone(), &(path.into())).await;
            } else if path.is_file().await {
                let arc_path: Arc<PathBuf> = Arc::new((&path).into());
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    {
                        let mut result_ref = result.lock();
                        result_ref.assets.push(arc_path.clone());
                        result_ref
                            .by_extension
                            .entry(ext.to_lowercase())
                            .or_default()
                            .push(arc_path.clone());
                    }

                    if ext == "asset" {
                        spawned_tasks.push(async_std::task::spawn(index_asset_meta(
                            root.clone(),
                            assets.clone(),
                            arc_path,
                            result.clone(),
                        )));
                    }
                } else {
                    result.lock().assets.push(arc_path);
                }
            }
        }

        for task in spawned_tasks {
            let _ = task.await;
        }

        result
    })
}

async fn index_asset_meta(
    root: PathBuf,
    assets: AssetCache,
    path: Arc<PathBuf>,
    result: Arc<Mutex<FileSystemIndex>>,
) {
    let meta =
        AssetPayloadUntyped::load_meta_from_file(&AssetPath::from_root(root, &*path), &assets)
            .await;

    match meta {
        Ok(meta) => {
            result
                .lock()
                .by_type
                .entry(meta.type_name)
                .or_default()
                .push(path);
        }
        Err(e) => {
            tracing::error!("Failed to index meta for asset: {path:?}\n\n{e:?}");
        }
    }
}

#[derive(Default)]
pub struct FileSystemIndex {
    pub assets: Vec<Arc<PathBuf>>,
    pub by_type: BTreeMap<String, Vec<Arc<PathBuf>>>,
    pub by_extension: BTreeMap<String, Vec<Arc<PathBuf>>>,
}

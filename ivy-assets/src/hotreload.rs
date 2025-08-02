use std::{path::PathBuf, sync::Arc};

use crate::{loadable::LoadFromPath, service::Service, AssetCache, AssetPath};
use async_std::stream::StreamExt;
use dashmap::DashMap;
use notify::Watcher;
use parking_lot::Mutex;

pub struct FileReloadService {
    tracked_paths: Arc<DashMap<PathBuf, Box<dyn Send + Sync + Fn(&AssetCache)>>>,

    notify: Mutex<notify::RecommendedWatcher>,
    rx: flume::Receiver<notify::Event>,
}

impl FileReloadService {
    pub fn new() -> anyhow::Result<Self> {
        let (tx, rx) = flume::unbounded();

        let watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            match res {
                Ok(event) => {
                    // Handle the event, e.g., log it or trigger a reload
                    let _ = tx.send(event);
                }
                Err(e) => {
                    tracing::error!("Error watching file changes: {}", e);
                }
            }
        })?;

        Ok(Self {
            tracked_paths: Arc::new(DashMap::new()),
            notify: Mutex::new(watcher),
            rx,
        })
    }

    pub fn track_path<T: 'static + LoadFromPath>(&self, path: AssetPath<T>) -> anyhow::Result<()> {
        match self.tracked_paths.entry(path.path().clone()) {
            dashmap::Entry::Occupied(_) => {}
            dashmap::Entry::Vacant(vacant_entry) => {
                self.notify
                    .lock()
                    .watch(path.path(), notify::RecursiveMode::NonRecursive)?;

                let path = path;
                vacant_entry.insert(Box::new(move |assets| {
                    let path = path.clone();
                    let assets = assets.clone();
                    async_std::task::spawn({
                        async move {
                            tracing::info!("Reloading asset {:?}", path);
                            let _ = assets.reload(&path).await;
                        }
                    });
                }));
            }
        }

        Ok(())
    }
}

impl Service for FileReloadService {
    fn register(&self, assets: &crate::AssetCache) {
        let assets = assets.clone();

        let mut rx = self.rx.clone().into_stream();
        let tracked_paths = self.tracked_paths.clone();
        async_std::task::spawn(async move {
            while let Some(event) = rx.next().await {
                // Process the event, e.g., reload the asset
                match event.kind {
                    notify::EventKind::Modify(_) => {
                        for path in &event.paths {
                            if let Some(callback) = tracked_paths.get(path) {
                                (callback.value())(&assets);
                            } else {
                                tracing::warn!("No callback registered for path: {:?}", path);
                            }
                        }
                    }
                    notify::EventKind::Create(_) => {
                        tracing::info!("File created: {:?}", event.paths);
                    }
                    notify::EventKind::Remove(_) => {
                        tracing::info!("File removed: {:?}", event.paths);
                    }
                    _ => {}
                }
            }
        });
    }
}

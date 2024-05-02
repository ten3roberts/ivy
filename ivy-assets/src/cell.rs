use std::sync::Arc;

use slotmap::SlotMap;

use super::{handle::WeakHandle, Asset, AssetId};

/// Contains the actual asset data
///
/// Allows accessing an asset by its id
pub(crate) struct AssetCell<V> {
    values: SlotMap<AssetId, WeakHandle<V>>,
}

impl<V> AssetCell<V> {
    pub(crate) fn new() -> Self {
        Self {
            values: SlotMap::with_key(),
        }
    }

    pub fn insert(&mut self, value: V) -> Asset<V> {
        if self.values.len() as f32 >= self.values.capacity() as f32 * 0.7 {
            self.prune();
        }

        let value = Arc::new(value);

        let id = self.values.insert_with_key(|id| WeakHandle {
            value: Arc::downgrade(&value),
            id,
        });

        Asset { value, id }
    }

    pub fn prune(&mut self) {
        self.values.retain(|_, v| v.strong_count() > 0)
    }
}

impl<V> Default for AssetCell<V> {
    fn default() -> Self {
        Self::new()
    }
}

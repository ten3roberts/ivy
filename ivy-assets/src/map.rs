use std::borrow::Borrow;

use slotmap::{
    secondary::{self},
    SecondaryMap,
};

use super::{Asset, AssetId};

/// Associates handles with locally owned data
pub struct AssetMap<K, V> {
    inner: SecondaryMap<AssetId, V>,
    _marker: std::marker::PhantomData<K>,
}

impl<K, V> AssetMap<K, V> {
    pub fn new() -> Self {
        Self {
            inner: SecondaryMap::new(),
            _marker: std::marker::PhantomData,
        }
    }

    pub fn insert<Q>(&mut self, handle: Q, value: V)
    where
        Q: Borrow<Asset<K>>,
    {
        self.inner.insert(handle.borrow().id(), value);
    }

    pub fn insert_with_id(&mut self, id: AssetId, value: V) {
        self.inner.insert(id, value);
    }

    pub fn get(&self, handle: &Asset<K>) -> Option<&V> {
        self.inner.get(handle.id())
    }

    pub fn get_mut(&mut self, handle: &Asset<K>) -> Option<&mut V> {
        self.inner.get_mut(handle.id())
    }

    pub fn remove(&mut self, handle: &Asset<K>) -> Option<V> {
        self.inner.remove(handle.id())
    }

    pub fn entry<'a>(&'a mut self, handle: &Asset<K>) -> secondary::Entry<'a, AssetId, V> {
        self.inner.entry(handle.id()).expect("Invalid handle")
    }

    pub fn iter(&self) -> secondary::Iter<'_, AssetId, V> {
        self.inner.iter()
    }

    pub fn iter_mut(&mut self) -> secondary::IterMut<'_, AssetId, V> {
        self.inner.iter_mut()
    }

    pub fn contains(&self, animation: &Asset<K>) -> bool {
        self.inner.contains_key(animation.id())
    }
}

impl<'a, K, V> IntoIterator for &'a AssetMap<K, V> {
    type Item = (AssetId, &'a V);

    type IntoIter = slotmap::secondary::Iter<'a, AssetId, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.inner.iter()
    }
}

impl<'a, K, V> IntoIterator for &'a mut AssetMap<K, V> {
    type Item = (AssetId, &'a mut V);

    type IntoIter = slotmap::secondary::IterMut<'a, AssetId, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.inner.iter_mut()
    }
}
// pub struct Entry<'a, 'h, K, V> {
//     entry: secondary::Entry<'a, AssetId, V>,
//     handle: &'h Asset<K>,
// }

// impl<'a, 'h, K, V> Entry<'a, 'h, K, V> {
//     pub fn and_modify<F>(self, f: F) -> Self
//     where
//         F: FnOnce(&mut V),
//     {
//         Entry {
//             entry: self.entry.and_modify(f),
//             handle: self.handle,
//         }
//     }

//     pub fn or_insert(self, default: V) -> &'a mut V {
//         self.entry.or_insert(default)
//     }

//     pub fn or_insert_with<F>(self, default: F) -> &'a mut V
//     where
//         F: FnOnce() -> V,
//     {
//         self.entry.or_insert_with(default)
//     }

//     pub fn or_insert_with<F>(self, default: F) -> &'a mut V
//     where
//         F: FnOnce() -> V,
//     {
//         self.entry.or_insert_with(default)
//     }
//     pub fn or_insert_with_key<F>(self, default: F) -> &'a mut V
//     where
//         F: FnOnce(&'h Asset<K>) -> V,
//     {
//         self.entry.or_insert_with(|| default(self.handle))
//     }
// }

impl<K, V> Default for AssetMap<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

use std::cmp::Ordering;
use std::{self};

use std::hash::Hash;

use std::sync::{Arc, Weak};

use super::AssetId;

#[derive(Debug)]
pub struct WeakHandle<T: ?Sized> {
    pub(crate) id: AssetId,
    pub(crate) value: Weak<T>,
}

impl<T: ?Sized> Clone for WeakHandle<T> {
    fn clone(&self) -> Self {
        Self {
            value: self.value.clone(),
            id: self.id,
        }
    }
}

impl<T: ?Sized> WeakHandle<T> {
    pub fn upgrade(&self) -> Option<Asset<T>> {
        self.value
            .upgrade()
            .map(|value| Asset { value, id: self.id })
    }

    pub fn strong_count(&self) -> usize {
        self.value.strong_count()
    }

    #[inline]
    pub fn id(&self) -> AssetId {
        self.id
    }
}

impl<T: ?Sized> PartialEq for WeakHandle<T> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl<T: ?Sized> Eq for WeakHandle<T> {}

/// Keep-alive handle to an asset
///
/// Works like an `Arc` with a unique identifier to allow it to be compared and sorted regardless of `T`.
pub struct Asset<T: ?Sized> {
    pub(crate) id: AssetId,
    pub(crate) value: Arc<T>,
}

impl<T: ?Sized> std::fmt::Debug for Asset<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Asset").field(&self.id()).finish()
    }
}

impl<T> AsRef<T> for Asset<T>
where
    T: ?Sized,
{
    fn as_ref(&self) -> &T {
        &self.value
    }
}

impl<T: ?Sized> std::ops::Deref for Asset<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T: ?Sized> Clone for Asset<T> {
    fn clone(&self) -> Self {
        Self {
            value: self.value.clone(),
            id: self.id,
        }
    }
}

impl<T: ?Sized> Asset<T> {
    pub fn downgrade(&self) -> WeakHandle<T> {
        WeakHandle {
            value: Arc::downgrade(&self.value),
            id: self.id,
        }
    }

    pub fn as_arc(&self) -> &Arc<T> {
        &self.value
    }

    #[inline]
    pub fn id(&self) -> AssetId {
        self.id
    }
}

impl<T: ?Sized> Hash for Asset<T> {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl<T: ?Sized> PartialOrd for Asset<T> {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T: ?Sized> Ord for Asset<T> {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        self.id.cmp(&other.id)
    }
}

impl<T: ?Sized> PartialEq for Asset<T> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl<T: ?Sized> Eq for Asset<T> {}

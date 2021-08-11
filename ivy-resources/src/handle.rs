use std::hash::Hash;
use std::marker::PhantomData;

use slotmap::{Key, KeyData};

#[derive()]
pub struct Handle<T>(KeyData, PhantomData<T>);

impl<T> Handle<T> {
    /// Creates a new handle that is always invalid and distinct from any non-null
    /// handle. A null key can only be created through this method (or default
    /// initialization of handles made with [`new_key_type!`], which calls this
    /// method).
    ///
    /// A null handle is always invalid, but an invalid key (that is, a key that
    /// has been removed from the slot map) does not become a null handle. A null
    /// is safe to use with any safe method of any slot map instance.
    pub fn null() -> Self {
        Key::null()
    }

    /// Checks if a handle is null. There is only a single null key, that is
    /// `a.is_null() && b.is_null()` implies `a == b`.
    pub fn is_null(&self) -> bool {
        Key::is_null(self)
    }
}

impl<T> Key for Handle<T> {
    fn data(&self) -> KeyData {
        self.0
    }
}

impl<T> PartialOrd for Handle<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.0.cmp(&other.0))
    }
}

impl<T> Ord for Handle<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.cmp(&other.0)
    }
}

impl<T> Default for Handle<T> {
    fn default() -> Self {
        Self(KeyData::default(), PhantomData)
    }
}

impl<T> std::fmt::Debug for Handle<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

impl<T> Clone for Handle<T> {
    fn clone(&self) -> Self {
        Self(self.0, PhantomData)
    }
}

impl<T> Copy for Handle<T> {}

impl<T> PartialEq for Handle<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<T> Eq for Handle<T> {}

impl<T> Hash for Handle<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state)
    }
}

impl<T> From<KeyData> for Handle<T> {
    fn from(k: KeyData) -> Self {
        Self(k, PhantomData)
    }
}

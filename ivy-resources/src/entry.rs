use std::ops::{Deref, DerefMut};

use crate::{CellRefMut, Handle, ResourceCache, Storage};

pub enum Entry<'a, T> {
    Occupied(&'a mut ResourceCache<T>, Handle<T>),
    Vacant(&'a mut ResourceCache<T>),
}

impl<'a, T: Storage> Entry<'a, T> {
    /// Retrieves the value if it exists or inserts the provided default.
    pub fn or_insert(self, default: T) -> HandleWrapper<T, &'a mut T> {
        match self {
            Self::Occupied(cache, handle) => HandleWrapper {
                handle,
                borrow: cache.get_mut(handle).unwrap(),
            },
            Self::Vacant(cache) => cache.insert_get(default),
        }
    }

    /// Retrieves the value if it exists or inserts the provided default from
    /// the given function.
    pub fn or_insert_with<F>(self, func: F) -> HandleWrapper<T, &'a mut T>
    where
        F: FnOnce() -> T,
    {
        match self {
            Self::Occupied(cache, handle) => HandleWrapper {
                handle,
                borrow: cache.get_mut(handle).unwrap(),
            },
            Self::Vacant(cache) => cache.insert_get(func()),
        }
    }

    /// Retrieves the value if it exists or inserts the provided default from
    /// the given fallible function.
    pub fn or_try_insert_with<F, E>(
        self,
        func: F,
    ) -> std::result::Result<HandleWrapper<T, &'a mut T>, E>
    where
        F: FnOnce() -> std::result::Result<T, E>,
    {
        match self {
            Self::Occupied(cache, handle) => Ok(HandleWrapper {
                handle,
                borrow: cache.get_mut(handle).unwrap(),
            }),
            Self::Vacant(cache) => {
                let val = func()?;
                Ok(cache.insert_get(val))
            }
        }
    }

    /// Modifies the value if it exists in the cache.
    pub fn and_modify<F: FnOnce(&T)>(mut self, func: F) -> Self {
        match &mut self {
            Self::Occupied(cache, val) => func(cache.get_mut(*val).unwrap()),
            Self::Vacant(_) => {}
        };
        self
    }

    /// Returns the value if it exists or the default value in the cache.
    /// Note: A default value may not exists for the cache, hence, result is
    /// returned.
    pub fn or_default(self) -> crate::Result<&'a mut T> {
        match self {
            Self::Occupied(cache, handle) => cache.get_mut(handle),
            Self::Vacant(cache) => cache.get_default_mut(),
        }
    }
}

pub enum RefEntry<'a, T> {
    Occupied(CellRefMut<'a, ResourceCache<T>>, Handle<T>),
    Vacant(CellRefMut<'a, ResourceCache<T>>),
}

impl<'a, T: Storage> RefEntry<'a, T> {
    /// Retrieves the value if it exists or inserts the provided value.
    pub fn or_insert(self, val: T) -> HandleWrapper<T, CellRefMut<'a, T>> {
        match self {
            Self::Occupied(cache, handle) => HandleWrapper {
                handle,
                borrow: cache.map(|cache| cache.get_mut(handle).unwrap()),
            },
            Self::Vacant(cache) => ResourceCache::insert_get_cell(cache, val),
        }
    }

    /// Retrieves the value if it exists or inserts the provided default from
    /// the given function.
    pub fn or_insert_with<F>(self, func: F) -> HandleWrapper<T, CellRefMut<'a, T>>
    where
        F: FnOnce() -> T,
    {
        match self {
            Self::Occupied(cache, handle) => HandleWrapper {
                handle,
                borrow: cache.map(|cache| cache.get_mut(handle).unwrap()),
            },
            Self::Vacant(cache) => ResourceCache::insert_get_cell(cache, func()),
        }
    }

    /// Retrieves the value if it exists or inserts the provided default from
    /// the given fallible function.
    pub fn or_try_insert_with<F, E>(
        self,
        func: F,
    ) -> std::result::Result<HandleWrapper<T, CellRefMut<'a, T>>, E>
    where
        F: FnOnce() -> std::result::Result<T, E>,
    {
        match self {
            Self::Occupied(cache, handle) => Ok(HandleWrapper {
                handle,
                borrow: cache.map(|cache| cache.get_mut(handle).unwrap()),
            }),
            Self::Vacant(cache) => {
                let val = func()?;
                Ok(ResourceCache::insert_get_cell(cache, val))
            }
        }
    }

    /// Modifies the value if it exists in the cache.
    pub fn and_modify<F: FnOnce(&mut T)>(mut self, func: F) -> Self {
        match &mut self {
            Self::Occupied(cache, val) => func(cache.get_mut(*val).unwrap()),
            Self::Vacant(_) => {}
        };
        self
    }

    /// Returns the value if it exists or the default value in the cache.
    /// Note: A default value may not exists for the cache, hence, result is
    /// returned.
    pub fn or_default(self) -> crate::Result<CellRefMut<'a, T>> {
        match self {
            Self::Occupied(cache, handle) => cache.try_map(|cache| cache.get_mut(handle)),
            Self::Vacant(cache) => cache.try_map(|cache| cache.get_default_mut()),
        }
    }
}

pub struct HandleWrapper<T, U> {
    pub handle: Handle<T>,
    pub borrow: U,
}

impl<T, U: Deref<Target = T>> Deref for HandleWrapper<T, U> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.borrow.deref()
    }
}

impl<T, U: DerefMut<Target = T>> DerefMut for HandleWrapper<T, U> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.borrow.deref_mut()
    }
}

use crate::{CellRefMut, Handle, ResourceCache, Storage};

pub enum Entry<'a, T> {
    Occupied(&'a mut ResourceCache<T>, Handle<T>),
    Vacant(&'a mut ResourceCache<T>),
}

impl<'a, T: Storage> Entry<'a, T> {
    /// Retrieves the value if it exists or inserts the provided default.
    pub fn or_insert(self, default: T) -> &'a mut T {
        match self {
            Self::Occupied(cache, handle) => cache.get_mut(handle).unwrap(),
            Self::Vacant(cache) => cache.insert_get(default),
        }
    }

    /// Retrieves the value if it exists or inserts the provided default from
    /// the given function.
    pub fn or_insert_with<F>(self, func: F) -> &'a mut T
    where
        F: FnOnce() -> T,
    {
        match self {
            Self::Occupied(cache, handle) => cache.get_mut(handle).unwrap(),
            Self::Vacant(cache) => cache.insert_get(func()),
        }
    }

    /// Retrieves the value if it exists or inserts the provided default from
    /// the given fallible function.
    pub fn or_try_insert_with<F, E>(self, func: F) -> std::result::Result<&'a mut T, E>
    where
        F: FnOnce() -> std::result::Result<T, E>,
    {
        match self {
            Self::Occupied(cache, handle) => Ok(cache.get_mut(handle).unwrap()),
            Self::Vacant(cache) => match func() {
                Ok(val) => Ok(cache.insert_get(val)),
                Err(e) => Err(e),
            },
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
    /// Retrieves the value if it exists or inserts the provided default.
    pub fn or_insert(self, default: T) -> CellRefMut<'a, T> {
        match self {
            Self::Occupied(cache, handle) => cache.map(|cache| cache.get_mut(handle).unwrap()),
            Self::Vacant(cache) => cache.map(|cache| cache.insert_get(default)),
        }
    }

    /// Retrieves the value if it exists or inserts the provided default from
    /// the given function.
    pub fn or_insert_with<F>(self, func: F) -> CellRefMut<'a, T>
    where
        F: FnOnce() -> T,
    {
        match self {
            Self::Occupied(cache, handle) => cache.map(|cache| cache.get_mut(handle).unwrap()),
            Self::Vacant(cache) => cache.map(|cache| cache.insert_get(func())),
        }
    }

    /// Retrieves the value if it exists or inserts the provided default from
    /// the given fallible function.
    pub fn or_try_insert_with<F, E>(self, func: F) -> std::result::Result<CellRefMut<'a, T>, E>
    where
        F: FnOnce() -> std::result::Result<T, E>,
    {
        match self {
            Self::Occupied(cache, handle) => Ok(cache.map(|cache| cache.get_mut(handle).unwrap())),
            Self::Vacant(cache) => cache.try_map(|cache| match func() {
                Ok(val) => Ok(cache.insert_get(val)),
                Err(e) => Err(e),
            }),
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

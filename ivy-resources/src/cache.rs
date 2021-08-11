use crate::Result;
use std::any::type_name;

use slotmap::SlotMap;

use crate::{Error, Handle};

pub trait Resource {}

/// Stores resources of a single type. Resources are inserted and accessed by a handle.
pub struct ResourceCache<T> {
    resources: SlotMap<Handle<T>, T>,
}

impl<T: 'static + Sized> ResourceCache<T> {
    pub fn new() -> Self {
        Self {
            resources: SlotMap::with_key(),
        }
    }

    pub fn insert(&mut self, resource: T) -> Handle<T> {
        self.resources.insert(resource).into()
    }

    pub fn get(&self, handle: Handle<T>) -> Result<&T> {
        self.resources
            .get(handle.into())
            .ok_or_else(|| Error::InvalidHandle(type_name::<T>()))
    }

    pub fn get_mut(&mut self, handle: Handle<T>) -> Result<&mut T> {
        self.resources
            .get_mut(handle.into())
            .ok_or_else(|| Error::InvalidHandle(type_name::<T>()))
    }
}

impl<T: 'static + Sized> Default for ResourceCache<T> {
    fn default() -> Self {
        Self::new()
    }
}

use std::{any::type_name, hash::Hash, marker::PhantomData};
use thiserror::Error;

use generational_arena::{Arena, Index};

pub trait Resource {}

#[derive(Debug)]
pub struct Handle<T>(Index, PhantomData<T>);

impl<T> Clone for Handle<T> {
    fn clone(&self) -> Self {
        Self(self.0, PhantomData)
    }
}

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

impl<T> Copy for Handle<T> {}

impl<T> From<Index> for Handle<T> {
    fn from(idx: Index) -> Self {
        Self(idx, PhantomData)
    }
}

impl<T> Into<Index> for &Handle<T> {
    fn into(self) -> Index {
        self.0
    }
}

impl<T> Into<Index> for Handle<T> {
    fn into(self) -> Index {
        self.0
    }
}

impl<T> AsRef<Index> for Handle<T> {
    fn as_ref(&self) -> &Index {
        &self.0
    }
}

/// Stores resources of a single type. Resources are inserted and accessed by a handle.
pub struct ResourceCache<T> {
    resources: Arena<T>,
}

impl<T: 'static + Sized> ResourceCache<T> {
    pub fn new() -> Self {
        Self {
            resources: Arena::new(),
        }
    }

    pub fn insert(&mut self, resource: T) -> Handle<T> {
        self.resources.insert(resource).into()
    }

    pub fn get(&self, handle: Handle<T>) -> Result<&T, Error> {
        self.resources
            .get(handle.into())
            .ok_or(Error::InvalidHandle(type_name::<T>()))
    }

    pub fn get_mut(&mut self, handle: Handle<T>) -> Result<&mut T, Error> {
        self.resources
            .get_mut(handle.into())
            .ok_or(Error::InvalidHandle(type_name::<T>()))
    }
}

#[derive(Debug, Error, Clone, Copy)]
pub enum Error {
    #[error("Invalid handle for {0}")]
    InvalidHandle(&'static str),
}

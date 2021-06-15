use std::hash::Hash;
use std::marker::PhantomData;

use generational_arena::Index;

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

impl<T> From<&Handle<T>> for Index {
    fn from(handle: &Handle<T>) -> Index {
        handle.0
    }
}

impl<T> From<Handle<T>> for Index {
    fn from(handle: Handle<T>) -> Index {
        handle.0
    }
}

impl<T> AsRef<Index> for Handle<T> {
    fn as_ref(&self) -> &Index {
        &self.0
    }
}

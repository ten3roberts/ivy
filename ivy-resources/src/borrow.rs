use atomic_refcell::AtomicRef;

use crate::{ResourceCache, Resources};

pub struct ResourceView<'a, T>(pub(crate) AtomicRef<'a, ResourceCache<T>>);

pub struct ResourceViewMut<'a, T> {
    value: *mut ResourceCache<T>,
    pub cell: AtomicRef<'a, Resources>,
}

impl<'a, T> std::ops::Deref for ResourceView<'a, T> {
    type Target = ResourceCache<T>;

    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}

impl<'a, T> std::ops::Deref for ResourceViewMut<'a, T> {
    type Target = ResourceCache<T>;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.value }
    }
}

impl<'a, T> std::ops::DerefMut for ResourceViewMut<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.value }
    }
}

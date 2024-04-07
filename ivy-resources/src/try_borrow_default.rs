use crate::{ResourceView, ResourceViewMut};
use smallvec::smallvec;

pub struct TryDefaultResource<'a, T> {
    value: Option<&'a T>,
    view: ResourceView<'a, T>,
}

impl<'a, T> TryDefaultResource<'a, T> {
    /// Get a reference to the default ref's view.
    pub fn view(&self) -> &ResourceView<'a, T> {
        &self.view
    }

    pub fn unwrap(self) -> &'a T {
        self.value.unwrap()
    }

    pub fn value(&self) -> Option<&'a T> {
        self.value
    }
}

pub struct TryDefaultResourceMut<'a, T> {
    value: Option<&'a mut T>,
    view: ResourceViewMut<'a, T>,
}

impl<'a, T> TryDefaultResourceMut<'a, T> {
    /// Get a reference to the default ref mut's view.
    pub fn view(&self) -> &ResourceViewMut<'a, T> {
        &self.view
    }

    pub fn unwrap(self) -> &'a T {
        self.value.unwrap()
    }

    pub fn value(self) -> Option<&'a mut T> {
        self.value
    }
}

impl<'a, T> std::ops::Deref for TryDefaultResource<'a, T> {
    type Target = Option<&'a T>;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<'a, T> std::ops::Deref for TryDefaultResourceMut<'a, T> {
    type Target = Option<&'a mut T>;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<'a, T> std::ops::DerefMut for TryDefaultResourceMut<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}

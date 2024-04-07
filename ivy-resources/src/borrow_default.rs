use crate::{ResourceView, ResourceViewMut};

pub struct DefaultResource<'a, T> {
    value: &'a T,
    view: ResourceView<'a, T>,
}

impl<'a, T> DefaultResource<'a, T> {
    /// Get a reference to the default ref's view.
    pub fn view(&self) -> &ResourceView<'a, T> {
        &self.view
    }
}

pub struct DefaultResourceMut<'a, T> {
    value: &'a mut T,
    view: ResourceViewMut<'a, T>,
}

impl<'a, T> DefaultResourceMut<'a, T> {
    /// Get a reference to the default ref mut's view.
    pub fn view(&self) -> &ResourceViewMut<'a, T> {
        &self.view
    }
}

impl<'a, T> std::ops::Deref for DefaultResource<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.value
    }
}

impl<'a, T> std::ops::Deref for DefaultResourceMut<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.value
    }
}

impl<'a, T> std::ops::DerefMut for DefaultResourceMut<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.value
    }
}

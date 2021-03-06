use std::ops::DerefMut;

use hecs::Component;
use hecs_schedule::{
    borrow::{Borrows, ComponentBorrow, ContextBorrow},
    impl_into_borrow, Access, Context, IntoAccess,
};

use crate::{BorrowMarker, ResourceView, ResourceViewMut};
use smallvec::smallvec;

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

impl<'a, T: Component> ContextBorrow<'a> for DefaultResource<'a, T> {
    type Target = Self;

    fn borrow(context: &'a Context) -> hecs_schedule::error::Result<Self::Target> {
        let view = ResourceView::<T>::borrow(context)?;

        let value = unsafe { &*(view.0.get_default().unwrap() as *const T) };
        Ok(DefaultResource { value, view })
    }
}
impl<'a, T: Component> ContextBorrow<'a> for DefaultResourceMut<'a, T> {
    type Target = Self;

    fn borrow(context: &'a Context) -> hecs_schedule::error::Result<Self::Target> {
        let mut view = ResourceViewMut::<T>::borrow(context)?;

        let value = unsafe { &mut *(view.deref_mut().get_default_mut().unwrap() as *mut T) };
        Ok(DefaultResourceMut { value, view })
    }
}

impl<'a, T: 'static> ComponentBorrow for DefaultResource<'a, T> {
    fn borrows() -> Borrows {
        smallvec![Access::of::<&BorrowMarker<T>>()]
    }

    fn has_dynamic(id: std::any::TypeId, exclusive: bool) -> bool {
        let l = Access::of::<&T>();

        l.id() == id && !exclusive
    }

    fn has<U: IntoAccess>() -> bool {
        Access::of::<&T>() == U::access()
    }
}

impl<'a, T: 'static> ComponentBorrow for DefaultResourceMut<'a, T> {
    fn borrows() -> Borrows {
        smallvec![Access::of::<&mut BorrowMarker<T>>()]
    }

    fn has_dynamic(id: std::any::TypeId, _: bool) -> bool {
        let l = Access::of::<&mut T>();

        l.id() == id
    }

    fn has<U: IntoAccess>() -> bool {
        Access::of::<&T>().id() == U::access().id()
    }
}

impl_into_borrow!(Component, DefaultResource => DefaultRefBorrow);
impl_into_borrow!(Component, DefaultResourceMut => DefaultRefMutBorrow);

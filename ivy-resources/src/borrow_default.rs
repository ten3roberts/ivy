use std::ops::DerefMut;

use hecs::Component;
use hecs_schedule::{
    borrow::{Borrows, ComponentBorrow, ContextBorrow},
    impl_into_borrow, Access, Context, IntoAccess,
};

use crate::{BorrowMarker, ResourceView, ResourceViewMut};
use smallvec::smallvec;

pub struct DefaultRef<'a, T> {
    value: *const T,
    view: ResourceView<'a, T>,
}

impl<'a, T> DefaultRef<'a, T> {
    /// Get a reference to the default ref's view.
    pub fn view(&self) -> &ResourceView<'a, T> {
        &self.view
    }
}

pub struct DefaultRefMut<'a, T> {
    value: *mut T,
    view: ResourceViewMut<'a, T>,
}

impl<'a, T> DefaultRefMut<'a, T> {
    /// Get a reference to the default ref mut's view.
    pub fn view(&self) -> &ResourceViewMut<'a, T> {
        &self.view
    }
}

impl<'a, T> std::ops::Deref for DefaultRef<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.value }
    }
}

impl<'a, T> std::ops::Deref for DefaultRefMut<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.value }
    }
}

impl<'a, T> std::ops::DerefMut for DefaultRefMut<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.value }
    }
}

impl<'a, T: Component> ContextBorrow<'a> for DefaultRef<'a, T> {
    type Target = Self;

    fn borrow(context: &'a Context) -> hecs_schedule::error::Result<Self::Target> {
        let view = ResourceView::<T>::borrow(context)?;

        let value = view.0.get_default().unwrap() as *const _;
        Ok(DefaultRef { value, view })
    }
}
impl<'a, T: Component> ContextBorrow<'a> for DefaultRefMut<'a, T> {
    type Target = Self;

    fn borrow(context: &'a Context) -> hecs_schedule::error::Result<Self::Target> {
        let mut view = ResourceViewMut::<T>::borrow(context)?;

        let value = view.deref_mut().get_default_mut().unwrap() as *mut _;
        Ok(DefaultRefMut { value, view })
    }
}

impl<'a, T: 'static> ComponentBorrow for DefaultRef<'a, T> {
    fn borrows() -> Borrows {
        smallvec![BorrowMarker::<&T>::access()]
    }

    fn has_dynamic(id: std::any::TypeId, exclusive: bool) -> bool {
        let l = Access::of::<&T>();

        l.id() == id && !exclusive
    }

    fn has<U: IntoAccess>() -> bool {
        Access::of::<&T>() == U::access()
    }
}

impl<'a, T: 'static> ComponentBorrow for DefaultRefMut<'a, T> {
    fn borrows() -> Borrows {
        smallvec![BorrowMarker::<&T>::access()]
    }

    fn has_dynamic(id: std::any::TypeId, _: bool) -> bool {
        let l = Access::of::<&T>();

        l.id() == id
    }

    fn has<U: IntoAccess>() -> bool {
        Access::of::<&T>().id() == U::access().id()
    }
}

impl_into_borrow!(Component, DefaultRef => DefaultRefBorrow);
impl_into_borrow!(Component, DefaultRefMut => DefaultRefMutBorrow);

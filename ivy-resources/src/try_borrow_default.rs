use hecs::Component;
use hecs_schedule::{
    borrow::{Borrows, ComponentBorrow, ContextBorrow},
    impl_into_borrow, Access, Context, IntoAccess,
};

use crate::{BorrowMarker, ResourceView, ResourceViewMut};
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

impl<'a, T: Component> ContextBorrow<'a> for TryDefaultResource<'a, T> {
    type Target = Self;

    fn borrow(context: &'a Context) -> hecs_schedule::error::Result<Self::Target> {
        let view = ResourceView::<T>::borrow(context)?;

        let value = match view.0.get_default() {
            Ok(val) => unsafe { Some(&*(val as *const T)) },
            Err(_) => None,
        };
        Ok(TryDefaultResource { value, view })
    }
}
impl<'a, T: Component> ContextBorrow<'a> for TryDefaultResourceMut<'a, T> {
    type Target = Self;

    fn borrow(context: &'a Context) -> hecs_schedule::error::Result<Self::Target> {
        let mut view = ResourceViewMut::<T>::borrow(context)?;

        let value = match view.get_default_mut() {
            Ok(val) => unsafe { Some(&mut *(val as *mut T)) },
            Err(_) => None,
        };
        Ok(TryDefaultResourceMut { value, view })
    }
}

impl<'a, T: 'static> ComponentBorrow for TryDefaultResource<'a, T> {
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

impl<'a, T: 'static> ComponentBorrow for TryDefaultResourceMut<'a, T> {
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

impl_into_borrow!(Component, TryDefaultResource => DefaultRefBorrow);
impl_into_borrow!(Component, TryDefaultResourceMut => DefaultRefMutBorrow);

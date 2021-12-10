use atomic_refcell::AtomicRef;
use derive_more::{From, Into};
use hecs::Component;
use hecs_schedule::{
    borrow::{ComponentBorrow, ContextBorrow},
    impl_into_borrow, *,
};
use smallvec::smallvec;
use std::{any::type_name, marker::PhantomData};

use crate::{ResourceCache, Resources};

#[derive(From, Into)]
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

impl<'a, T: Component> ContextBorrow<'a> for ResourceView<'a, T> {
    type Target = Self;

    fn borrow(context: &'a Context) -> hecs_schedule::error::Result<Self::Target> {
        context
            .cell::<&Resources>()?
            .try_borrow()
            .map_err(|_| hecs_schedule::Error::Borrow(type_name::<T>()))
            .map(|cell| {
                AtomicRef::map(cell, |cell| unsafe {
                    cell.cast::<Resources>()
                        .as_ref()
                        .fetch()
                        .expect("Failed to borrow from resources")
                        .value
                })
                .into()
            })
    }
}

pub(crate) struct BorrowMarker<T>(PhantomData<T>);

impl<'a, T: 'static> ComponentBorrow for ResourceView<'a, T> {
    fn borrows() -> borrow::Borrows {
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

impl<'a, T: Component> ContextBorrow<'a> for ResourceViewMut<'a, T> {
    type Target = Self;

    fn borrow(context: &'a Context) -> hecs_schedule::error::Result<Self::Target> {
        context
            .cell::<&Resources>()?
            .try_borrow()
            .map_err(|_| hecs_schedule::Error::Borrow(type_name::<T>()))
            .map(|cell| {
                let cell =
                    AtomicRef::map(cell, |cell| unsafe { cell.cast::<Resources>().as_ref() });

                let value = cell
                    .fetch_mut()
                    .expect("Failed to borrow from resources mutably")
                    .value as *mut _;

                ResourceViewMut { value, cell }
            })
    }
}

impl<'a, T: 'static> ComponentBorrow for ResourceViewMut<'a, T> {
    fn borrows() -> borrow::Borrows {
        smallvec![Access::of::<&mut BorrowMarker<T>>()]
    }

    fn has_dynamic(id: std::any::TypeId, _: bool) -> bool {
        let l = Access::of::<&mut T>();

        l.id() == id
    }

    fn has<U: IntoAccess>() -> bool {
        Access::of::<&mut T>().id() == U::access().id()
    }
}

impl_into_borrow!(Component, ResourceView => ResourcesBorrow);
impl_into_borrow!(Component, ResourceViewMut => ResourcesBorrowMut);

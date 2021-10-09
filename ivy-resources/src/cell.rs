use crate::{Error, Result};
use atomic_refcell::{AtomicRef, AtomicRefCell, AtomicRefMut};
use std::{
    any::type_name,
    marker::PhantomData,
    mem,
    ops::{Deref, DerefMut},
};

// Remove generics of inner cell.
// The AtomicRefCell is boxed and a raw pointer is acquired. This is needed to circumvent borrowed
// temporary inside RwLockGuard. Since cells are never removed partially from the ResourceManager,
// this is safe.
// The pointer is valid for the entire lifetime of cell.
pub struct Cell {
    pub(crate) inner: *mut AtomicRefCell<dyn Storage>,
}

// Since Storage implements Send + Sync, this is safe.
unsafe impl Send for Cell {}
// unsafe impl Sync for Cell {}

impl Cell {
    // A raw pointer to the internally boxed AtomicRefCell is stored. The pointer is
    // valid as long as self because caches can never be individually removed
    pub fn new<T: Storage>(value: T) -> Self {
        Self {
            inner: Box::into_raw(Box::new(AtomicRefCell::new(value))),
        }
    }

    pub fn borrow<'a, T: Storage>(&self) -> Result<CellRef<'a, T>> {
        let borrow = unsafe {
            (*self.inner)
                .try_borrow()
                .map_err(|_| Error::Borrow(type_name::<T>()))?
        };

        CellRef::new(borrow)
    }

    pub fn borrow_mut<'a, T: Storage>(&self) -> Result<CellRefMut<'a, T>> {
        let borrow = unsafe {
            (*self.inner)
                .try_borrow_mut()
                .map_err(|_| Error::BorrowMut(type_name::<T>()))?
        };

        CellRefMut::new(borrow)
    }
}

impl Drop for Cell {
    fn drop(&mut self) {
        unsafe { mem::drop(Box::from_raw(self.inner)) };
    }
}

pub struct CellRef<'a, T> {
    value: &'a T,
    borrow: AtomicRef<'a, dyn Storage>,
    marker: PhantomData<T>,
}

impl<'a, T> CellRef<'a, T>
where
    T: 'static,
{
    #[inline]
    pub fn new(borrow: AtomicRef<'a, dyn Storage>) -> Result<Self> {
        let data = borrow
            .deref()
            .as_any()
            .downcast_ref::<T>()
            .expect("Failed to downcast cell") as *const _;

        Ok(Self {
            value: unsafe { &*data },
            borrow,
            marker: PhantomData,
        })
    }

    // Transforms the borrowed cell.
    #[inline]
    pub fn map<U: 'static, F: FnOnce(&T) -> &U>(self, f: F) -> CellRef<'a, U> {
        CellRef {
            value: f(self.value),
            borrow: self.borrow,
            marker: PhantomData,
        }
    }

    // Fallible version of [`map`].
    #[inline]
    pub fn try_map<U: 'static, F: FnOnce(&T) -> std::result::Result<&U, E>, E>(
        self,
        f: F,
    ) -> std::result::Result<CellRef<'a, U>, E> {
        Ok(CellRef {
            value: f(self.value)?,
            borrow: self.borrow,
            marker: PhantomData,
        })
    }
}

impl<'a, T> Deref for CellRef<'a, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.value
    }
}

pub struct CellRefMut<'a, T> {
    value: &'a mut T,
    borrow: AtomicRefMut<'a, dyn Storage>,
    marker: PhantomData<T>,
}

impl<'a, T> CellRefMut<'a, T>
where
    T: 'static,
{
    #[inline]
    pub fn new(mut borrow: AtomicRefMut<'a, dyn Storage>) -> Result<Self> {
        let data = borrow
            .deref_mut()
            .as_any_mut()
            .downcast_mut::<T>()
            .expect("Failed to downcast cell") as *mut _;

        Ok(Self {
            value: unsafe { &mut *data },
            borrow,
            marker: PhantomData,
        })
    }

    // Transforms the borrowed cell.
    #[inline]
    pub fn map<U: 'static, F: FnOnce(&mut T) -> &mut U>(self, f: F) -> CellRefMut<'a, U> {
        CellRefMut {
            value: f(self.value),
            borrow: self.borrow,
            marker: PhantomData,
        }
    }

    // Fallible version of [`map`].
    #[inline]
    pub fn try_map<U: 'static, F: FnOnce(&mut T) -> std::result::Result<&mut U, E>, E>(
        self,
        f: F,
    ) -> std::result::Result<CellRefMut<'a, U>, E> {
        Ok(CellRefMut {
            value: f(self.value)?,
            borrow: self.borrow,
            marker: PhantomData,
        })
    }
}

impl<'a, T> Deref for CellRefMut<'a, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.value
    }
}

impl<'a, T> DerefMut for CellRefMut<'a, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.value
    }
}

pub trait Storage: 'static + Send {
    fn as_any(&self) -> &dyn std::any::Any;

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
}

impl<T> Storage for T
where
    T: 'static + Sized + Send,
{
    #[inline]
    fn as_any(&self) -> &dyn std::any::Any {
        self as &dyn std::any::Any
    }

    #[inline]
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self as &mut dyn std::any::Any
    }
}

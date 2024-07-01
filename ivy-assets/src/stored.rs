//! Slab based storage for non-send and non-sync types
use std::{
    any::{Any, TypeId},
    collections::HashMap,
    fmt::Debug,
    marker::PhantomData,
    ops::{Index, IndexMut},
    sync::{Arc, Weak},
};

use slotmap::{new_key_type, SecondaryMap, SlotMap};

new_key_type! {
    pub struct HandleIndex;
}

/// Allows storing non-send and non-sync types through handles
pub struct Store<T> {
    values: SlotMap<HandleIndex, T>,
    refs: SecondaryMap<HandleIndex, Weak<()>>,
    free_tx: flume::Sender<HandleIndex>,
    free_rx: flume::Receiver<HandleIndex>,
}

impl<T> Default for Store<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Debug> Debug for Store<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.values.fmt(f)
    }
}

impl<T> Store<T> {
    pub fn new() -> Self {
        let (free_tx, free_rx) = flume::unbounded();
        Self {
            values: SlotMap::with_key(),
            free_tx,
            free_rx,
            refs: SecondaryMap::new(),
        }
    }

    pub fn reclaim(&mut self) {
        for index in self.free_rx.try_iter() {
            self.values.remove(index);
            self.refs.remove(index);
        }
    }

    pub fn insert(&mut self, value: T) -> Handle<T> {
        self.reclaim();
        let index = self.values.insert(value);
        let refs = Arc::new(());
        self.refs.insert(index, Arc::downgrade(&refs));

        Handle {
            index,
            free_tx: self.free_tx.clone(),
            refs,
            _marker: PhantomData,
        }
    }

    pub fn get(&self, handle: &Handle<T>) -> &T {
        self.values.get(handle.index).unwrap()
    }

    pub fn get_mut(&mut self, handle: &Handle<T>) -> &mut T {
        self.values.get_mut(handle.index).unwrap()
    }

    pub fn iter(&self) -> impl Iterator<Item = (HandleIndex, &T)> {
        self.values.iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (HandleIndex, &mut T)> {
        self.values.iter_mut()
    }
}

impl<T> Index<&Handle<T>> for Store<T> {
    type Output = T;

    fn index(&self, handle: &Handle<T>) -> &Self::Output {
        self.get(handle)
    }
}

impl<T> IndexMut<&Handle<T>> for Store<T> {
    fn index_mut(&mut self, handle: &Handle<T>) -> &mut Self::Output {
        self.get_mut(handle)
    }
}

impl<T> Index<&WeakHandle<T>> for Store<T> {
    type Output = T;

    fn index(&self, handle: &WeakHandle<T>) -> &Self::Output {
        self.get(&handle.upgrade(self).unwrap())
    }
}

impl<T> IndexMut<&WeakHandle<T>> for Store<T> {
    fn index_mut(&mut self, handle: &WeakHandle<T>) -> &mut Self::Output {
        self.get_mut(&handle.upgrade(self).unwrap())
    }
}

/// Reference counted handle to a value in a store
///
/// When the handle is dropped, the value is removed from the store
pub struct Handle<T> {
    index: HandleIndex,
    free_tx: flume::Sender<HandleIndex>,
    _marker: PhantomData<T>,
    refs: Arc<()>,
}

// Safety: no instance of T is stored
unsafe impl<T> Send for Handle<T> {}
unsafe impl<T> Sync for Handle<T> {}
unsafe impl<T> Send for WeakHandle<T> {}
unsafe impl<T> Sync for WeakHandle<T> {}

impl<T> Handle<T> {
    pub fn downgrade(&self) -> WeakHandle<T> {
        WeakHandle {
            index: self.index,
            _marker: PhantomData,
        }
    }
}

impl<T> Drop for Handle<T> {
    fn drop(&mut self) {
        if Arc::strong_count(&self.refs) == 1 {
            self.free_tx.send(self.index).ok();
        }
    }
}

impl<T> Clone for Handle<T> {
    fn clone(&self) -> Self {
        Self {
            index: self.index,
            free_tx: self.free_tx.clone(),
            _marker: PhantomData,
            refs: self.refs.clone(),
        }
    }
}

impl<T> PartialEq for Handle<T> {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index
    }
}

impl<T> Eq for Handle<T> {}
impl<T> Debug for Handle<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Handle").field(&self.index).finish()
    }
}

/// Cheap to clone handle to a value in a store
///
/// When the handle is dropped, the value is removed from the store
pub struct UntypedHandle {
    index: HandleIndex,
    free_tx: flume::Sender<HandleIndex>,
    ty: TypeId,
    refs: Arc<()>,
}

impl UntypedHandle {
    pub fn new<T: 'static>(handle: Handle<T>) -> Self {
        Self {
            index: handle.index,
            free_tx: handle.free_tx.clone(),
            ty: TypeId::of::<T>(),
            refs: handle.refs.clone(),
        }
    }

    pub fn downgrade(&self) -> WeakUntypedHandle {
        WeakUntypedHandle {
            index: self.index,
            ty: self.ty,
            _marker: PhantomData,
        }
    }

    pub fn downcast<T: 'static>(&self) -> Option<Handle<T>> {
        if self.ty == TypeId::of::<T>() {
            Some(Handle {
                index: self.index,
                free_tx: self.free_tx.clone(),
                _marker: PhantomData,
                refs: self.refs.clone(),
            })
        } else {
            None
        }
    }
}

impl Drop for UntypedHandle {
    fn drop(&mut self) {
        if Arc::strong_count(&self.refs) == 1 {
            self.free_tx.send(self.index).ok();
        }
    }
}

impl Clone for UntypedHandle {
    fn clone(&self) -> Self {
        Self {
            index: self.index,
            free_tx: self.free_tx.clone(),
            ty: self.ty,
            refs: self.refs.clone(),
        }
    }
}

impl PartialEq for UntypedHandle {
    fn eq(&self, other: &Self) -> bool {
        self.ty == other.ty && self.index == other.index
    }
}

impl Eq for UntypedHandle {}

pub struct WeakUntypedHandle {
    index: HandleIndex,
    ty: TypeId,
    _marker: PhantomData<*const ()>,
}

impl WeakUntypedHandle {
    pub fn downcast<T: 'static>(&self) -> Option<WeakHandle<T>> {
        if self.ty == TypeId::of::<T>() {
            Some(WeakHandle {
                index: self.index,
                _marker: PhantomData,
            })
        } else {
            None
        }
    }
}

pub struct WeakHandle<T> {
    index: HandleIndex,
    _marker: PhantomData<T>,
}

impl<T> WeakHandle<T> {
    pub fn upgrade(&self, store: &Store<T>) -> Option<Handle<T>> {
        let refs = store.refs.get(self.index)?.upgrade()?;
        Some(Handle {
            index: self.index,
            free_tx: store.free_tx.clone(),
            _marker: self._marker,
            refs,
        })
    }
}

impl<T> Copy for WeakHandle<T> {}

impl<T> Clone for WeakHandle<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> PartialEq for WeakHandle<T> {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index
    }
}

impl<T> Eq for WeakHandle<T> {}
impl<T> Debug for WeakHandle<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Handle").field(&self.index).finish()
    }
}

pub struct DynamicStore {
    inner: HashMap<TypeId, Box<dyn Any>>,
}

impl DynamicStore {
    pub fn new() -> Self {
        Self {
            inner: HashMap::new(),
        }
    }

    pub fn store_mut<T: 'static>(&mut self) -> &mut Store<T> {
        self.inner
            .entry(TypeId::of::<T>())
            .or_insert_with(|| Box::new(Store::<T>::new()))
            .downcast_mut::<Store<T>>()
            .unwrap()
    }

    pub fn store<T: 'static>(&self) -> Option<&Store<T>> {
        self.inner.get(&TypeId::of::<T>()).map(|v| {
            v.downcast_ref::<Store<T>>()
                .expect("DynamicStore: downcast failed")
        })
    }

    pub fn get<T: 'static>(&self, handle: &Handle<T>) -> &T {
        self.store::<T>().expect("Invalid handle").get(handle)
    }

    pub fn get_mut<T: 'static>(&mut self, handle: &Handle<T>) -> &mut T {
        self.store_mut::<T>().get_mut(handle)
    }

    pub fn insert<T: 'static>(&mut self, value: T) -> Handle<T> {
        self.store_mut::<T>().insert(value)
    }
}

impl<T: 'static> Index<&Handle<T>> for DynamicStore {
    type Output = T;

    fn index(&self, handle: &Handle<T>) -> &Self::Output {
        self.get(handle)
    }
}

impl<T: 'static> IndexMut<&Handle<T>> for DynamicStore {
    fn index_mut(&mut self, handle: &Handle<T>) -> &mut Self::Output {
        self.get_mut(handle)
    }
}

impl Default for DynamicStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_store() {
        let mut store = Store::new();
        let a = store.insert("Foo".to_string());
        let b = store.insert("Bar".to_string());

        let a_weak = a.downgrade();
        let b_weak = b.downgrade();

        assert_eq!(store.get(&a), "Foo");

        assert_eq!(store.get(&b), "Bar");
        drop(b);
        let a2 = a_weak.upgrade(&store).unwrap();
        assert!(b_weak.upgrade(&store).is_none());

        assert_eq!(a, a2);
        assert_eq!(store.get(&a), "Foo");
    }
}

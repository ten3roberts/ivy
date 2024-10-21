use std::{
    any::{Any, TypeId},
    collections::HashMap,
};

use downcast_rs::{impl_downcast, Downcast};
use flax::World;
use ivy_assets::AssetCache;
use ivy_profiling::{profile_function, profile_scope};

use crate::{Layer, LayerDyn};

type EventCallback<T> =
    Box<dyn Fn(&mut dyn LayerDyn, &mut World, &mut AssetCache, &T) -> anyhow::Result<bool>>;

pub struct EventDispatcher<T> {
    listeners: Vec<(usize, EventCallback<T>)>,
}

impl<T> EventDispatcher<T> {
    pub fn new() -> Self {
        Self {
            listeners: Vec::new(),
        }
    }

    pub fn dispatch(
        &self,
        layers: &mut [Box<dyn LayerDyn>],
        world: &mut World,
        assets: &mut AssetCache,
        event: &T,
    ) -> anyhow::Result<()> {
        for (layer_index, func) in &self.listeners {
            let layer = &mut layers[*layer_index];
            profile_scope!("dispatch_layer", layer.label());
            let handled = func(layer.as_mut(), world, assets, event)?;

            if handled {
                break;
            }
        }

        Ok(())
    }

    pub fn register<L: Layer>(
        &mut self,
        layer_index: usize,
        callback: impl 'static + Fn(&mut L, &mut World, &mut AssetCache, &T) -> anyhow::Result<bool>,
    ) {
        self.listeners.push((
            layer_index,
            Box::new(move |layer, world, assets, event| {
                let layer = layer.downcast_mut::<L>().expect("Failed to downcast layer");

                callback(layer, world, assets, event)
            }),
        ));
    }
}

impl<T> Default for EventDispatcher<T> {
    fn default() -> Self {
        Self::new()
    }
}

pub struct EventRegistry {
    dispatchers: HashMap<std::any::TypeId, Box<dyn Any>>,
}

impl EventRegistry {
    pub fn new() -> Self {
        Self {
            dispatchers: HashMap::new(),
        }
    }

    pub fn get<T: 'static>(&self) -> Option<&EventDispatcher<T>> {
        self.dispatchers
            .get(&std::any::TypeId::of::<T>())
            .map(|dispatcher| dispatcher.downcast_ref::<EventDispatcher<T>>().unwrap())
    }

    pub fn get_or_insert<T: Event>(&mut self) -> &mut EventDispatcher<T> {
        self.dispatchers
            .entry(TypeId::of::<T>())
            .or_insert_with(|| Box::new(EventDispatcher::<T>::new()))
            .downcast_mut::<EventDispatcher<T>>()
            .unwrap()
    }

    pub fn emit<T: Event>(
        &self,
        layers: &mut [Box<dyn LayerDyn>],
        world: &mut World,
        assets: &mut AssetCache,
        event: &T,
    ) -> anyhow::Result<()> {
        profile_function!(std::any::type_name::<T>());
        if let Some(dispatcher) = self.get::<T>() {
            dispatcher.dispatch(layers, world, assets, event)?;
        }

        Ok(())
    }
}

impl Default for EventRegistry {
    fn default() -> Self {
        Self::new()
    }
}

pub struct EventRegisterContext<'a, L> {
    pub(crate) registry: &'a mut EventRegistry,
    index: usize,
    _marker: std::marker::PhantomData<L>,
}

impl<'a, L: Layer> EventRegisterContext<'a, L> {
    pub fn new(registry: &'a mut EventRegistry, index: usize) -> Self {
        Self {
            registry,
            index,
            _marker: std::marker::PhantomData,
        }
    }

    /// Register an event callback for the given event type.
    pub fn subscribe<T: Event>(
        &mut self,
        callback: impl 'static + Fn(&mut L, &mut World, &mut AssetCache, &T) -> anyhow::Result<()>,
    ) {
        self.registry.get_or_insert::<T>().register(
            self.index,
            move |layer, world, assets, value| {
                callback(layer, world, assets, value)?;
                Ok(false)
            },
        );
    }

    /// Allows intercepting and controlling the control flow of an event
    pub fn intercept<T: Event>(
        &mut self,
        callback: impl 'static + Fn(&mut L, &mut World, &mut AssetCache, &T) -> anyhow::Result<bool>,
    ) {
        self.registry
            .get_or_insert::<T>()
            .register(self.index, callback);
    }
}

pub trait Event: 'static + std::fmt::Debug + Downcast {}
impl_downcast!(Event);

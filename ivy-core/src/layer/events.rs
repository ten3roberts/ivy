use std::{any::TypeId, collections::HashMap};

use downcast_rs::{impl_downcast, Downcast};
use flax::World;
use ivy_assets::AssetCache;
use ivy_profiling::{profile_function, profile_scope};
use slab::Slab;

use crate::{Layer, LayerDyn};

type EventCallbackDyn =
    Box<dyn FnMut(&mut dyn LayerDyn, &mut World, &AssetCache, &dyn Event) -> anyhow::Result<bool>>;

pub struct Callbacks {
    callbacks: Slab<EventCallbackDyn>,
}

impl Callbacks {
    fn new() -> Self {
        Self {
            callbacks: Default::default(),
        }
    }

    fn register_callback(&mut self, callback: EventCallbackDyn) -> usize {
        self.callbacks.insert(callback)
    }
}

pub struct EventDispatcher {
    listeners: Vec<(usize, usize)>,
}

impl EventDispatcher {
    pub fn new() -> Self {
        Self {
            listeners: Vec::new(),
        }
    }

    pub fn dispatch(
        &self,
        layers: &mut [Box<dyn LayerDyn>],
        world: &mut World,
        assets: &AssetCache,
        registry: &mut Callbacks,
        event: &dyn Event,
    ) -> anyhow::Result<bool> {
        for (layer_index, func) in &self.listeners {
            let layer = &mut layers[*layer_index];
            profile_scope!("dispatch_layer", layer.label());
            let handled = registry.callbacks[*func](layer.as_mut(), world, assets, event)?;

            if handled {
                return Ok(handled);
            }
        }

        Ok(false)
    }

    pub fn register(&mut self, layer_index: usize, callback: usize) {
        self.listeners.push((layer_index, callback));
        self.listeners.sort_by_key(|v| v.0);
    }
}

impl Default for EventDispatcher {
    fn default() -> Self {
        Self::new()
    }
}

pub struct EventRegistry {
    dispatchers: HashMap<TypeId, EventDispatcher>,
    callbacks: Callbacks,
    // layer, callback
    global_listeners: EventDispatcher,
}

impl EventRegistry {
    pub fn new() -> Self {
        Self {
            dispatchers: HashMap::new(),
            callbacks: Callbacks::new(),
            global_listeners: EventDispatcher::new(),
        }
    }

    pub fn get<T: 'static>(&self) -> Option<&EventDispatcher> {
        self.dispatchers.get(&TypeId::of::<T>())
    }

    pub fn get_or_insert<T: Event>(&mut self) -> &mut EventDispatcher {
        self.dispatchers
            .entry(TypeId::of::<T>())
            .or_insert_with(|| {
                let mut dispatcher = EventDispatcher::new();
                for &(layer, callback) in &self.global_listeners.listeners {
                    dispatcher.register(layer, callback);
                }

                dispatcher
            })
    }

    fn register_global(&mut self, layer_index: usize, callback: usize) {
        for dispatcher in self.dispatchers.values_mut() {
            dispatcher.register(layer_index, callback)
        }

        self.global_listeners.register(layer_index, callback);
    }

    pub fn emit<T: Event>(
        &mut self,
        layers: &mut [Box<dyn LayerDyn>],
        world: &mut World,
        assets: &AssetCache,
        event: &T,
    ) -> anyhow::Result<()> {
        profile_function!(std::any::type_name::<T>());

        if let Some(dispatcher) = self.dispatchers.get(&TypeId::of::<T>()) {
            dispatcher.dispatch(layers, world, assets, &mut self.callbacks, event)?;
        } else {
            self.global_listeners
                .dispatch(layers, world, assets, &mut self.callbacks, event)?;
        }

        Ok(())
    }

    pub fn emit_dyn(
        &mut self,
        layers: &mut [Box<dyn LayerDyn>],
        world: &mut World,
        assets: &AssetCache,
        event: &dyn Event,
    ) -> anyhow::Result<bool> {
        profile_function!(event.type_name());

        let ty = event.type_id();
        if let Some(dispatcher) = self.dispatchers.get(&ty) {
            dispatcher.dispatch(layers, world, assets, &mut self.callbacks, event)
        } else {
            self.global_listeners
                .dispatch(layers, world, assets, &mut self.callbacks, event)
        }
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
        mut callback: impl 'static + FnMut(&mut L, &mut World, &AssetCache, &T) -> anyhow::Result<()>,
    ) {
        let callback = self.registry.callbacks.register_callback(Box::new(
            move |layer, world, assets, value| {
                let layer = layer.downcast_mut::<L>().unwrap();
                callback(layer, world, assets, value.downcast_ref().unwrap())?;
                Ok(false)
            },
        ));

        self.registry
            .get_or_insert::<T>()
            .register(self.index, callback);
    }

    /// Allows intercepting and controlling the control flow of an event
    pub fn intercept<T: Event>(
        &mut self,
        callback: impl 'static + Fn(&mut L, &mut World, &AssetCache, &T) -> anyhow::Result<bool>,
    ) {
        let callback = self.registry.callbacks.register_callback(Box::new(
            move |layer, world, assets, value| {
                let layer = layer.downcast_mut::<L>().unwrap();
                callback(layer, world, assets, value.downcast_ref().unwrap())
            },
        ));

        self.registry
            .get_or_insert::<T>()
            .register(self.index, callback);
    }

    /// Register an event callback for all event types
    pub fn subscribe_global(
        &mut self,
        callback: impl 'static + Fn(&mut L, &mut World, &AssetCache, &dyn Event) -> anyhow::Result<bool>,
    ) {
        let callback = self.registry.callbacks.register_callback(Box::new(
            move |layer, world, assets, value| {
                let layer = layer.downcast_mut::<L>().unwrap();
                callback(layer, world, assets, value)
            },
        ));

        self.registry.register_global(self.index, callback);
    }
}

pub trait Event: 'static + std::fmt::Debug + Downcast {
    fn type_name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

impl_downcast!(Event);

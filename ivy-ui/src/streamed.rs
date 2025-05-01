use std::pin::Pin;

use async_std::stream::StreamExt;
use flax::{
    component::ComponentValue,
    filter::{All, ChangeFilter},
    BoxedSystem, Component, ComponentMut, Entity, FetchExt, Query, System, World,
};
use futures::{FutureExt, Stream};
use ivy_assets::AssetCache;
use ivy_core::{
    components::engine,
    update_layer::{Plugin, ScheduleSetBuilder},
};
use violet::{
    core::{Scope, ScopeRef},
    futures_signals::signal::{Mutable, MutableSignalCloned, SignalExt, SignalStream},
};

flax::component! {
    pub streamed: Vec<Box<dyn Streamed>>,

    pub streamed_tx: flume::Sender<Box<dyn Streamed>>,
}

pub trait StreamedUiExt {
    fn open_streamed(&self, streamed: impl Streamed);

    fn stream_component<T: ComponentValue + Clone>(
        &self,
        component: Component<T>,
        target: Entity,
    ) -> flume::Receiver<T> {
        let (tx, rx) = flume::unbounded();

        self.open_streamed(StreamedComponent::new(component, target, tx));
        rx
    }

    fn stream_component_ref<T: ComponentValue, U: 'static + Send>(
        &self,
        component: Component<T>,
        target: Entity,
        map: impl 'static + Send + Sync + FnMut(&T) -> U,
    ) -> flume::Receiver<U> {
        let (tx, rx) = flume::unbounded();

        self.open_streamed(StreamedComponentRef::new(component, target, map, tx));
        rx
    }

    fn stream_component_duplex<T: ComponentValue + Clone + PartialEq>(
        &self,
        component: Component<T>,
        target: Entity,
        state: Mutable<Option<T>>,
    ) {
        self.open_streamed(DuplexComponentStream::new(component, target, state));
    }

    fn monitor_entity_lifetime(
        &self,
        target: Entity,
        on_despawned: impl 'static + Send + Sync + FnMut(),
    ) {
        self.open_streamed(StreamEntityLifetime::new(target, on_despawned));
    }

    fn apply(&self, func: impl 'static + Send + Sync + FnOnce(&World) -> anyhow::Result<()>) {
        self.open_streamed(StreamedAction { action: Some(func) });
    }
}

impl StreamedUiExt for Scope<'_> {
    fn open_streamed(&self, streamed: impl Streamed) {
        let context = self.get_context(streamed_tx());

        context.send(Box::new(streamed)).expect("Channel closed");
    }
}

impl StreamedUiExt for ScopeRef<'_> {
    fn open_streamed(&self, streamed: impl Streamed) {
        let context = self.get_context(streamed_tx());

        context.send(Box::new(streamed)).expect("Channel closed");
    }
}

pub fn update_streamed_system(rx: flume::Receiver<Box<dyn Streamed>>) -> BoxedSystem {
    System::builder()
        .with_world()
        .with_query(Query::new(streamed().as_mut()).with_filter(engine()))
        .build(
            move |world: &World,
             mut query: flax::QueryBorrow<'_, ComponentMut<Vec<Box<dyn Streamed>>>, _>| {
                let streamed_values = query.first().unwrap();

                streamed_values.extend(rx.try_iter());
                streamed_values.retain_mut(|v| v.update(world))
            },
        )
        .boxed()
}

/// Allows streaming custom state between world/scene and UI
pub trait Streamed: 'static + Send + Sync {
    fn update(&mut self, world: &World) -> bool;
}

pub struct StreamedAction<F> {
    action: Option<F>,
}

impl<F: 'static + Send + Sync + FnOnce(&World) -> anyhow::Result<()>> Streamed
    for StreamedAction<F>
{
    fn update(&mut self, world: &World) -> bool {
        if let Some(action) = self.action.take() {
            if let Err(err) = (action)(world) {
                tracing::error!("StreamedAction: {err:?}");
            }
        }

        false
    }
}

/// Allows for writing to a component from the UI
pub struct ComponentSink<T, S> {
    target: Entity,
    component: Component<T>,
    tx: Pin<Box<S>>,
}

impl<T: ComponentValue, S: 'static + Send + Sync + Stream<Item = T>> ComponentSink<T, S> {
    pub fn new(component: Component<T>, target: Entity, tx: S) -> Self {
        Self {
            target,
            component,
            tx: Box::pin(tx),
        }
    }
}

impl<T: ComponentValue, S: 'static + Send + Sync + Stream<Item = T>> Streamed
    for ComponentSink<T, S>
{
    fn update(&mut self, world: &World) -> bool {
        if !world.is_alive(self.target) {
            return false;
        }

        if let Some(value) = self.tx.next().now_or_never() {
            let Some(value) = value else {
                return false;
            };

            *world.get_mut(self.target, self.component).unwrap() = value;
        }

        true
    }
}

pub struct DuplexComponentStream<T, F> {
    state: Mutable<Option<T>>,
    signal: SignalStream<MutableSignalCloned<Option<T>>>,
    target: Entity,
    component: Component<T>,
    query: Query<ChangeFilter<T>, (All, Entity)>,
    compare: F,
}

impl<T: Clone + ComponentValue + PartialEq> DuplexComponentStream<T, fn(&T, &T) -> bool> {
    pub fn new(component: Component<T>, target: Entity, state: Mutable<Option<T>>) -> Self {
        Self {
            signal: state.signal_cloned().to_stream(),
            state,
            target,
            component,
            query: Query::new(component.modified()).with_filter(target),
            compare: |a, b| a == b,
        }
    }
}

impl<T, F> DuplexComponentStream<T, F>
where
    T: ComponentValue + Clone,
    F: 'static + Send + Sync + Fn(&T, &T) -> bool,
{
    pub fn new_with_compare(
        component: Component<T>,
        target: Entity,
        state: Mutable<Option<T>>,
        compare: F,
    ) -> Self {
        Self {
            signal: state.signal_cloned().to_stream(),
            state,
            target,
            component,
            query: Query::new(component.modified()).with_filter(target),
            compare,
        }
    }
}

impl<T, F> Streamed for DuplexComponentStream<T, F>
where
    T: ComponentValue + Clone,
    F: 'static + Send + Sync + Fn(&T, &T) -> bool,
{
    fn update(&mut self, world: &World) -> bool {
        if let Ok(value) = self.query.borrow(world).get(self.target) {
            tracing::info!("{:?} changed", self.component);
            self.state.set(Some(value.clone()));
        }

        if let Some(Some(Some(val))) = self.signal.next().now_or_never() {
            {
                let Ok(mut value) = world.get_mut(self.target, self.component) else {
                    return false;
                };

                if (self.compare)(&value, &val) {
                    return true;
                }

                *value = val;
            }

            let tick = world.change_tick();
            self.query.set_change_tick(tick);
        }

        true
    }
}

pub struct StreamEntityLifetime<F> {
    target: Entity,
    on_despawned: F,
}

impl<F> StreamEntityLifetime<F> {
    pub fn new(target: Entity, on_despawned: F) -> Self {
        Self {
            target,
            on_despawned,
        }
    }
}

impl<F> Streamed for StreamEntityLifetime<F>
where
    F: 'static + Send + Sync + FnMut(),
{
    fn update(&mut self, world: &World) -> bool {
        if !world.is_alive(self.target) {
            (self.on_despawned)();
            false
        } else {
            true
        }
    }
}

pub struct StreamedComponent<T> {
    tx: flume::Sender<T>,
    target: Entity,
    query: Query<ChangeFilter<T>, (All, Entity)>,
}

impl<T: Clone + ComponentValue> StreamedComponent<T> {
    pub fn new(component: Component<T>, target: Entity, tx: flume::Sender<T>) -> Self {
        Self {
            tx,
            target,
            query: Query::new(component.modified()).with_filter(target),
        }
    }
}

impl<T: ComponentValue + Clone> Streamed for StreamedComponent<T> {
    fn update(&mut self, world: &World) -> bool {
        if let Ok(value) = self.query.borrow(world).get(self.target) {
            self.tx.send(value.clone()).is_ok()
        } else {
            !self.tx.is_disconnected()
        }
    }
}

pub struct StreamedComponentRef<T, U, F> {
    tx: flume::Sender<U>,
    target: Entity,
    query: Query<ChangeFilter<T>, (All, Entity)>,
    map: F,
}

impl<T, U, F> StreamedComponentRef<T, U, F>
where
    T: ComponentValue,
    U: 'static + Send,
    F: 'static + Send + Sync + FnMut(&T) -> U,
{
    pub fn new(component: Component<T>, target: Entity, map: F, tx: flume::Sender<U>) -> Self {
        Self {
            tx,
            target,
            query: Query::new(component.modified()).with_filter(target),
            map,
        }
    }
}

impl<T, U, F> Streamed for StreamedComponentRef<T, U, F>
where
    T: ComponentValue,
    U: 'static + Send,
    F: 'static + Send + Sync + FnMut(&T) -> U,
{
    fn update(&mut self, world: &World) -> bool {
        if let Ok(value) = self.query.borrow(world).get(self.target) {
            self.tx.send((self.map)(value)).is_ok()
        } else {
            !self.tx.is_disconnected()
        }
    }
}

/// Streams ECS data from world into UI
pub struct StreamedUiPlugin;

impl Plugin for StreamedUiPlugin {
    fn install(
        &self,
        world: &mut World,
        _: &AssetCache,
        schedules: &mut ScheduleSetBuilder,
    ) -> anyhow::Result<()> {
        let (tx, rx) = flume::unbounded();
        world.set(engine(), streamed_tx(), tx)?;
        world.set(engine(), streamed(), Default::default())?;

        schedules
            .per_tick_mut()
            .with_system(update_streamed_system(rx));

        Ok(())
    }
}

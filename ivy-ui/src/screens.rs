use std::sync::Arc;

use flax::{component, Entity, EntityRef, World};
use glam::Vec2;
use ivy_core::{components::engine, update_layer::Plugin};
use parking_lot::Mutex;
use slotmap::{SecondaryMap, SlotMap};
use violet::{
    core::{
        components::children,
        input::{interactive, keep_focus, request_focus_sender},
        layout::Align,
        style::SizeExt,
        widget::{maximized, Stack},
        Scope, Widget,
    },
    glam,
};

pub enum ScreenCommand {
    Open(ScreenId, bool, Box<dyn Send + FnOnce() -> Box<dyn Widget>>),
    Replace(ScreenId, Box<dyn Send + FnOnce() -> Box<dyn Widget>>),
    Close(ScreenId),
}

impl ScreenCommand {
    pub fn open<W: 'static + Widget>(
        id: ScreenId,
        ctor: impl 'static + Send + FnOnce() -> W,
        top: bool,
    ) -> Self {
        Self::Open(id, top, Box::new(|| Box::new(ctor())))
    }
}

pub trait Screen: 'static + Send {
    fn create(self, scope: &mut Scope<'_>, token: ScreenLifetimeToken);

    fn block_lower_input(&self) -> bool {
        false
    }
}

#[derive(Clone)]
pub struct ScreenLifetimeToken {
    id: ScreenId,
    commands: flume::Sender<ScreenCommand>,
}

impl ScreenLifetimeToken {
    pub fn new(id: ScreenId, commands: flume::Sender<ScreenCommand>) -> Self {
        Self { id, commands }
    }

    pub fn replace(&self, screen: impl Screen) {
        let token = self.clone();
        let _ = self.commands.send(ScreenCommand::Replace(
            self.id,
            Box::new(move || Box::new(ScreenWidget { screen, token })),
        ));
    }

    pub fn close_screen(&self) {
        let _ = self.commands.send(ScreenCommand::Close(self.id));
    }
}

slotmap::new_key_type! {
    pub struct ScreenId;
}

/// Allows managing the screen stack
#[derive(Clone)]
pub struct ScreenState {
    inner: Arc<ScreenStateInner>,
}

pub struct ScreenStateInner {
    screens: Mutex<SlotMap<ScreenId, ()>>,
    cmd_tx: flume::Sender<ScreenCommand>,
    rx: flume::Receiver<ScreenCommand>,
}

impl ScreenState {
    pub fn new() -> Self {
        let (tx, rx) = flume::unbounded();
        Self {
            inner: Arc::new(ScreenStateInner {
                screens: Default::default(),
                cmd_tx: tx,
                rx,
            }),
        }
    }

    pub fn open(&self, screen: impl Screen) -> ScreenLifetimeToken {
        let id = self.inner.screens.lock().insert(());
        let token = ScreenLifetimeToken::new(id, self.inner.cmd_tx.clone());
        let token2 = token.clone();

        let _ = self.inner.cmd_tx.send(ScreenCommand::open(
            id,
            move || ScreenWidget { screen, token },
            true,
        ));

        token2
    }

    pub fn open_below(&self, screen: impl Screen) -> ScreenLifetimeToken {
        let id = self.inner.screens.lock().insert(());
        let token = ScreenLifetimeToken::new(id, self.inner.cmd_tx.clone());
        let token2 = token.clone();

        let _ = self.inner.cmd_tx.send(ScreenCommand::open(
            id,
            move || ScreenWidget { screen, token },
            false,
        ));

        token2
    }

    pub fn close<W: 'static + Widget>(&self, id: ScreenId) {
        let _ = self.inner.cmd_tx.send(ScreenCommand::Close(id));
    }
}

impl Default for ScreenState {
    fn default() -> Self {
        Self::new()
    }
}

/// Main plugin for installing the screen stack
pub struct ScreenPlugin;

impl Plugin for ScreenPlugin {
    fn install(
        &self,
        world: &mut World,
        _: &ivy_assets::AssetCache,
        _: &mut ivy_assets::stored::DynamicStore,
        _: &mut ivy_core::update_layer::ScheduleSetBuilder,
    ) -> anyhow::Result<()> {
        world.set(engine(), screen_state(), ScreenState::new())?;

        Ok(())
    }
}

component! {
    pub screen_state: ScreenState,
}

/// Manages opening and closing of toplevel screens/windows in the UI
pub struct ScreenStack {
    screens: SecondaryMap<ScreenId, Entity>,
    state: ScreenState,
}

impl ScreenStack {
    pub fn new(state: ScreenState) -> Self {
        Self {
            screens: Default::default(),
            state,
        }
    }

    pub fn with_screen(&mut self, screen: impl Screen) -> &mut Self {
        self.state.open(screen);
        self
    }

    pub fn state(&self) -> &ScreenState {
        &self.state
    }
}

impl Widget for ScreenStack {
    fn mount(mut self, scope: &mut Scope<'_>) {
        let rx = self.state.inner.rx.clone();
        scope.set_context(screen_state(), self.state);

        let request_focus = scope.get_atom(request_focus_sender()).unwrap().clone();

        scope.spawn_stream(rx.into_stream(), move |scope, item| {
            match item {
                ScreenCommand::Open(screen_id, top, ctor) => {
                    let widget = ctor();
                    let entity = if top {
                        scope.attach(widget)
                    } else {
                        scope.attach_at(0, widget)
                    };

                    self.screens.insert(screen_id, entity);
                }
                ScreenCommand::Replace(screen_id, ctor) => {
                    let Some(screen_entity) = self.screens.get_mut(screen_id) else {
                        return;
                    };

                    let index = scope
                        .children()
                        .iter()
                        .position(|&v| v == *screen_entity)
                        .expect("Screen not a child");

                    scope.detach(*screen_entity);
                    let entity = scope.attach_at(index, ctor());
                    *screen_entity = entity;
                }
                ScreenCommand::Close(screen_id) => {
                    let Some(&screen_entity) = self.screens.get(screen_id) else {
                        return;
                    };

                    scope.detach(screen_entity);
                }
            }

            scope.flush();

            for &child in scope.children().iter().rev() {
                let child = scope.frame().world().entity(child).unwrap();

                let to_focus = find_in_tree(scope.frame().world(), child, &|v| {
                    v.has(self::request_focus())
                })
                .unwrap_or(child);

                request_focus.send(to_focus.id()).unwrap();

                if child.has(block_lower_input()) {
                    tracing::info!("blocking input");
                    break;
                }
            }
        });

        Stack::new(())
            .with_horizontal_alignment(Align::Center)
            .with_vertical_alignment(Align::Center)
            .with_maximize(Vec2::ONE)
            .mount(scope);
    }
}

struct ScreenWidget<S> {
    screen: S,
    token: ScreenLifetimeToken,
}

impl<S: Screen> Widget for ScreenWidget<S> {
    fn mount(self, scope: &mut Scope<'_>) {
        let block_lower_input = self.screen.block_lower_input();

        if block_lower_input {
            scope.set_default(self::block_lower_input());
        }

        if block_lower_input {
            scope.attach(move |scope: &mut Scope| {
                self.screen.create(scope, self.token);
            });

            scope.set_default(interactive());

            maximized(())
                .with_vertical_alignment(Align::Center)
                .with_horizontal_alignment(Align::Center)
                .mount(scope);
        } else {
            self.screen.create(scope, self.token);
        };
    }
}

component! {
    pub request_focus: (),
    block_lower_input: (),
}

fn find_in_tree<'a>(
    world: &'a World,
    root: EntityRef<'a>,
    f: &impl Fn(&EntityRef) -> bool,
) -> Option<EntityRef<'a>> {
    if f(&root) {
        return Some(root);
    }

    for &child in root.get(children()).as_deref().into_iter().flatten() {
        let entity = world.entity(child).unwrap();
        if let Some(v) = find_in_tree(world, entity, f) {
            return Some(v);
        }
    }

    None
}

pub struct EmptyScreen;

impl Screen for EmptyScreen {
    fn create(self, _: &mut Scope<'_>, _: ScreenLifetimeToken) {}
}

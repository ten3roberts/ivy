use std::{any::type_name, sync::Arc};

use enum_dispatch::enum_dispatch;
use flax::{Component, Entity, Query, QueryBorrow, System, World, component, system};
use ivy_core::{
    EntityBuilderExt,
    components::{engine, gizmos, position, rotation, world_transform},
    gizmos::{CuboidGizmo, DEFAULT_THICKNESS, Gizmos},
    template::Template,
    update_layer::Plugin,
};
use ivy_input::{
    Action, CompositeBinding, InputState, KeyBinding,
    components::input_state,
    types::{Key, NamedKey},
};
use ivy_physics::{
    components::{collider_handle, physics_state},
    state::PhysicsState,
};
use ivy_scene::{
    editor::manipulator::EntityManipulation,
    ray_picker::{RayPickerBundle, RayPickingPlugin},
};
use ivy_ui::{
    screens::screen_state,
    violet::{
        core::style::base_colors::EMERALD_400,
        lucide::icons::{
            LUCIDE_MOUSE_POINTER_2, LUCIDE_MOUSE_POINTER_CLICK, LUCIDE_MOVE_3D, LUCIDE_TORNADO,
        },
    },
};

use crate::{
    tools::{
        physics_tool::{PhysicsToolBundle, PhysicsToolPlugin, PhysicsToolWidget},
        select_tool::SelectToolBundle,
        transform_tool::{TransformToolBundle, TransformToolPlugin, TransformToolWidget},
    },
    tools_controller::{Tool, ToolsControllerBundle, ToolsControllerPlugin},
    ui::EditorUi,
};

#[enum_dispatch(CommandExt)]
pub enum EditCommand {
    SetSelection(SetSelection),
    MoveEntities(MoveEntities),
}

#[derive(Clone)]
pub struct SetSelection {
    pub new_selection: Selection,
}

#[derive(Clone)]
pub struct MoveEntities {
    pub items: Vec<EntityManipulation>,
}

impl MoveEntities {
    pub fn new(items: Vec<EntityManipulation>) -> Self {
        Self { items }
    }
}

/// Transactional commands for performing an action on the world
pub trait Command {
    type Undo: 'static + Send + Sync + Command;
    fn execute(&self, editor: Entity, world: &mut World) -> anyhow::Result<Self::Undo>;
}

#[enum_dispatch]
pub trait CommandExt: Send + Sync {
    fn execute_dyn(&self, editor: Entity, world: &mut World)
    -> anyhow::Result<Box<dyn CommandExt>>;
}

impl<T> CommandExt for T
where
    T: Send + Sync + Command,
{
    fn execute_dyn(
        &self,
        editor: Entity,
        world: &mut World,
    ) -> anyhow::Result<Box<dyn CommandExt>> {
        Ok(Box::new(self.execute(editor, world)?) as Box<dyn CommandExt>)
    }
}

impl Command for SetSelection {
    type Undo = Self;
    fn execute(&self, editor: Entity, world: &mut World) -> anyhow::Result<Self> {
        let old_selection = world.get_clone(editor, selection()).unwrap_or_default();

        world.set(editor, selection(), self.new_selection.clone())?;

        Ok(SetSelection {
            new_selection: old_selection,
        })
    }
}

impl Command for MoveEntities {
    type Undo = Self;
    fn execute(&self, _: Entity, world: &mut World) -> anyhow::Result<Self> {
        let mut undo = Vec::new();
        for manipulation in &self.items {
            undo.push(EntityManipulation {
                entity: manipulation.entity,
                original_position: manipulation.new_position,
                original_rotation: manipulation.new_rotation,
                new_position: manipulation.original_position,
                new_rotation: manipulation.original_rotation,
            });

            let Ok(entity) = world.entity(manipulation.entity) else {
                continue;
            };

            entity.update_dedup(position(), manipulation.new_position);
            entity.update_dedup(rotation(), manipulation.new_rotation);
        }

        Ok(Self::new(undo))
    }
}

pub struct HistoryManager {
    command_stack: Vec<Box<dyn CommandExt>>,
    redo_stack: Vec<Box<dyn CommandExt>>,
}

impl Default for HistoryManager {
    fn default() -> Self {
        Self::new()
    }
}

impl HistoryManager {
    pub fn new() -> Self {
        Self {
            command_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    pub fn execute(
        editor: Entity,
        world: &mut World,
        command: impl CommandExt,
    ) -> anyhow::Result<()> {
        let undo_command = command.execute_dyn(editor, world)?;

        let editor = &mut *world.get_mut(editor, editor_host())?;
        editor.history.command_stack.push(undo_command);
        editor.history.redo_stack.clear();
        Ok(())
    }

    pub fn undo(editor_id: Entity, world: &mut World) -> anyhow::Result<()> {
        let mut editor = world.get_mut(editor_id, editor_host())?;
        let command = editor.history.command_stack.pop();
        drop(editor);

        if let Some(command) = command {
            let undo_command = command.execute_dyn(editor_id, world)?;
            let editor = &mut world.get_mut(editor_id, editor_host())?;
            editor.history.redo_stack.push(undo_command);
        }
        Ok(())
    }

    pub fn redo(editor_id: Entity, world: &mut World) -> anyhow::Result<()> {
        let mut editor = world.get_mut(editor_id, editor_host())?;
        let command = editor.history.redo_stack.pop();
        drop(editor);

        if let Some(command) = command {
            let command = command.execute_dyn(editor_id, world)?;
            let editor = &mut world.get_mut(editor_id, editor_host())?;
            editor.history.command_stack.push(command);
        }
        Ok(())
    }
}

pub struct EditorPlugin;

impl Plugin for EditorPlugin {
    fn install(
        &self,
        world: &mut World,
        assets: &ivy_assets::AssetCache,
        store: &mut ivy_assets::stored::DynamicStore,
        schedules: &mut ivy_core::update_layer::ScheduleSetBuilder,
    ) -> anyhow::Result<()> {
        let (tx, rx) = flume::unbounded();

        let (tool_ui_tx, tool_ui_rx) = flume::unbounded();

        let host = EditorHost::new();

        let tools = vec![
            Tool {
                name: "Selection Tool".into(),
                icon: LUCIDE_MOUSE_POINTER_2.into(),
                template: assets.insert(Template::new().with_bundle(SelectToolBundle::new())),
                ui: None,
            },
            Tool {
                name: "Transform Tool".into(),
                icon: LUCIDE_MOVE_3D.into(),
                template: assets.insert(
                    Template::new().with_bundle(TransformToolBundle::new(Default::default())),
                ),
                ui: Some(Arc::new(|id| Box::new(TransformToolWidget::new(id)))),
            },
            Tool {
                name: "Drag Tool".into(),
                icon: LUCIDE_MOUSE_POINTER_CLICK.into(),
                template: assets.insert(Template::new().with_bundle(RayPickerBundle::new())),
                ui: None,
            },
            Tool {
                name: "Physics Data".into(),
                icon: LUCIDE_TORNADO.into(),
                template: assets.insert(
                    Template::new()
                        .with_bundle(PhysicsToolBundle)
                        .with_bundle(SelectToolBundle::new()),
                ),
                ui: Some(Arc::new(|id| Box::new(PhysicsToolWidget::new(id)))),
            },
        ];

        let input = InputState::new()
            .with_trigger_action(
                Action::new().with_binding(CompositeBinding::new(
                    KeyBinding::new(Key::Character("z".into())),
                    vec![KeyBinding::new(Key::Named(NamedKey::Control))],
                )),
                |entity, cmd, pressed| {
                    let id = entity.id();
                    if pressed {
                        cmd.defer(move |world| HistoryManager::undo(id, world));
                    }

                    Ok(())
                },
            )
            .with_trigger_action(
                Action::new().with_binding(CompositeBinding::new(
                    KeyBinding::new(Key::Character("y".into())),
                    vec![KeyBinding::new(Key::Named(NamedKey::Control))],
                )),
                |entity, cmd, pressed| {
                    let id = entity.id();
                    if pressed {
                        cmd.defer(move |world| HistoryManager::redo(id, world));
                    }

                    Ok(())
                },
            );

        let editor = Entity::builder()
            .set(selection(), Selection::new(vec![]))
            .set(editor_host(), host)
            .set(edit_commands(), tx)
            .set(input_state(), input)
            .mount(ToolsControllerBundle::new(tools, tool_ui_tx))
            .spawn(world);

        let process_commands_system = System::builder()
            .with_name("process_commands")
            .with_world_mut()
            .build(move |world: &mut World| -> anyhow::Result<()> {
                for command in rx.try_iter() {
                    HistoryManager::execute(editor, world, command)?;
                }

                Ok(())
            })
            .boxed();

        ToolsControllerPlugin.install(world, assets, store, schedules)?;

        schedules
            .per_tick_mut()
            .with_system(process_commands_system)
            .with_system(draw_selection_system());

        world.get(engine(), screen_state())?.open(EditorUi::new(
            assets.clone(),
            editor,
            tool_ui_rx,
        ));

        Ok(())
    }

    fn after(&self) -> Vec<&str> {
        vec![
            type_name::<TransformToolPlugin>(),
            type_name::<RayPickingPlugin>(),
            type_name::<PhysicsToolPlugin>(),
            type_name::<ToolsControllerPlugin>(),
        ]
    }
}

component! {
    pub edit_commands: flume::Sender<EditCommand>,
    pub selection: Selection,
    editor_host: EditorHost,
}

pub struct EditorHost {
    history: HistoryManager,
}

impl Default for EditorHost {
    fn default() -> Self {
        Self::new()
    }
}

impl EditorHost {
    pub fn new() -> Self {
        Self {
            history: HistoryManager::new(),
        }
    }
}

#[system(with_world, with_query(Query::new(selection())))]
fn draw_selection_system(
    gizmos: &Gizmos,
    physics_state: &PhysicsState,
    world: &World,
    query: &mut QueryBorrow<Component<Selection>>,
) {
    let mut gizmos = gizmos.begin_section("EditorPlugin::draw_selection_system");

    for selection in query {
        for entity in &selection.entities {
            let Ok(entity) = world.entity(*entity) else {
                continue;
            };

            let collider = entity.get_copy(collider_handle()).ok();

            let world_transform = entity.get_copy(world_transform()).unwrap_or_default();

            if let Some(collider) = collider {
                let bounds = physics_state
                    .collider(collider)
                    .shape()
                    .compute_local_aabb();
                gizmos.draw(
                    CuboidGizmo::new(
                        bounds.mins.into(),
                        bounds.maxs.into(),
                        DEFAULT_THICKNESS,
                        EMERALD_400,
                    )
                    .with_transform(world_transform),
                );
            }
        }
    }
}

#[derive(Default, Clone, Debug, PartialEq, Eq)]
pub struct Selection {
    entities: Vec<Entity>,
}

impl Selection {
    pub fn new(entities: Vec<Entity>) -> Self {
        Self { entities }
    }

    pub fn single(entity: Entity) -> Self {
        Self {
            entities: vec![entity],
        }
    }

    pub fn entities(&self) -> &[Entity] {
        &self.entities
    }

    pub fn append(&self, id: Entity) -> Selection {
        Self {
            entities: self
                .entities
                .iter()
                .filter(|&&e| e != id)
                .cloned()
                .chain([id])
                .collect(),
        }
    }

    pub fn append_toggle(&self, id: Entity) -> Selection {
        if self.entities.contains(&id) {
            Self {
                entities: self
                    .entities
                    .iter()
                    .filter(|&&e| e != id)
                    .cloned()
                    .collect(),
            }
        } else {
            self.append(id)
        }
    }
}

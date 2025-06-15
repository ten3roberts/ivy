use std::{any::type_name, sync::Arc};

use enum_dispatch::enum_dispatch;
use flax::{Component, Entity, Query, QueryBorrow, System, World, component, system};
use glam::{Quat, Vec3};
use ivy_core::{
    EntityBuilderExt,
    components::{TransformBundle, engine, gizmos, position, rotation, scale, world_transform},
    gizmos::{CuboidGizmo, DEFAULT_THICKNESS, Gizmos},
    update_layer::Plugin,
};
use ivy_physics::{
    components::{collider_handle, physics_state},
    state::PhysicsState,
};
use ivy_scene::{
    ray_picker::{RayPickerBundle, RayPickingPlugin},
    template::Template,
};
use ivy_ui::{
    screens::screen_state,
    violet::{
        core::style::base_colors::EMERALD_400,
        lucide::icons::{
            LUCIDE_MOUSE_POINTER_2, LUCIDE_MOUSE_POINTER_CLICK, LUCIDE_MOVE, LUCIDE_MOVE_3D,
            LUCIDE_TORNADO,
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

#[enum_dispatch(Command)]
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
    pub items: Vec<(Entity, Vec3, Quat)>,
}

impl MoveEntities {
    pub fn new(items: Vec<(Entity, Vec3, Quat)>) -> Self {
        Self { items }
    }
}

/// Transactional commands for performing an action on the world
#[enum_dispatch]
pub trait Command {
    fn execute(&mut self, editor: Entity, world: &mut World) -> anyhow::Result<()>;
}

impl Command for SetSelection {
    fn execute(&mut self, editor: Entity, world: &mut World) -> anyhow::Result<()> {
        world.set(editor, selection(), self.new_selection.clone())?;

        Ok(())
    }
}

impl Command for MoveEntities {
    fn execute(&mut self, _: Entity, world: &mut World) -> anyhow::Result<()> {
        for &(entity, pos, rot) in &self.items {
            let Ok(entity) = world.entity(entity) else {
                continue;
            };

            entity.update_dedup(position(), pos);
            entity.update_dedup(rotation(), rot);
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

        let editor = Entity::builder()
            .set(selection(), Selection::new(vec![]))
            .set(editor_host(), host)
            .set(edit_commands(), tx)
            .mount(ToolsControllerBundle::new(tools, tool_ui_tx))
            .spawn(world);

        let process_commands_system = System::builder()
            .with_name("process_commands")
            .with_world_mut()
            .build(move |world: &mut World| -> anyhow::Result<()> {
                for mut command in rx.try_iter() {
                    command.execute(editor, world)?;
                }

                Ok(())
            })
            .boxed();

        ToolsControllerPlugin.install(world, assets, store, schedules)?;

        schedules
            .per_tick_mut()
            .with_system(process_commands_system)
            .with_system(draw_selection_system());

        world
            .get(engine(), screen_state())?
            .open(EditorUi::new(editor, tool_ui_rx));

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

pub struct EditorHost {}

impl EditorHost {
    pub fn new() -> Self {
        Self {}
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

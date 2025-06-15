use std::sync::Arc;

use flax::{
    CommandBuffer, Entity, FetchExt, World, component, components::child_of, entity::EntityKind,
    signal::BoxedSignal, system,
};
use ivy_assets::{Asset, AssetCache};
use ivy_core::{
    Bundle,
    update_layer::{Plugin, ScheduleSetBuilder},
};
use ivy_input::{InputState, components::input_state};
use ivy_scene::template::Template;
use ivy_ui::violet::core::{
    Widget,
    widget::{card, col, label, row},
};

component! {
    tools_controller: ToolsController,
    pub(crate) current_tool: Option<usize>,
    pub(crate) tools: Vec<Tool>,
    unequip_signal: BoxedSignal<()>,
}

/// Defines a tool
#[derive(Clone)]
pub struct Tool {
    pub name: String,
    pub icon: String,
    pub template: Asset<Template>,
    pub ui: Option<Arc<dyn Send + Sync + Fn(Entity) -> Box<dyn Send + Widget>>>,
}

impl std::fmt::Debug for Tool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Tool")
            .field("name", &self.name)
            .field("icon", &self.icon)
            .field("template", &self.template)
            .finish()
    }
}

impl Tool {}

pub struct EquippedTool {
    name: String,
    id: Entity,
    template: Asset<Template>,
}

/// Handles equipping and managing tools in the editor.
pub struct ToolsController {
    current_tool: Option<EquippedTool>,
    open_tool_ui: flume::Sender<Box<dyn Send + Widget>>,
}

impl ToolsController {
    pub fn new(open_tool_ui: flume::Sender<Box<dyn Send + Widget>>) -> Self {
        Self {
            current_tool: None,
            open_tool_ui,
        }
    }

    #[system(args(current_tool=current_tool().copied().modified()), with_world, with_cmd_mut)]
    fn equip_tool_system(
        self: &mut ToolsController,
        id: Entity,
        tools: &[Tool],
        current_tool: Option<usize>,
        world: &World,
        cmd: &mut CommandBuffer,
    ) -> anyhow::Result<()> {
        let Some(current_tool) = current_tool else {
            return Ok(());
        };

        let tool = &tools[current_tool];

        if Some(&tool.template) == self.current_tool.as_ref().map(|v| &v.template) {
            return Ok(());
        }

        if let Some(tool) = self.current_tool.take() {
            let entity = world.entity(tool.id)?;
            if let Ok(mut unequip_signal) = entity.get_mut(unequip_signal()) {
                unequip_signal.execute(entity, cmd, ())?;
            }

            cmd.despawn_recursive(child_of, tool.id);
        }

        let tool_entity = world.reserve_one(EntityKind::empty());

        let mut builder = tool.template.build();
        builder.set_default(child_of(id));

        let widget = tool
            .ui
            .as_ref()
            .map(|ui| ui(tool_entity) as Box<dyn Send + Widget>);

        let widget = Box::new(card(col((
            row([label(tool.icon.clone()), label(tool.name.clone())]),
            widget,
        ))));

        let sender = self.open_tool_ui.clone();

        cmd.defer(move |world| {
            builder.append_to(world, tool_entity)?;
            let _ = sender.send(widget);

            Ok(())
        });

        self.current_tool = Some(EquippedTool {
            id: tool_entity,
            template: tool.template.clone(),
            name: tool.name.clone(),
        });

        Ok(())
    }
}

pub struct ToolsControllerBundle {
    tools: Vec<Tool>,
    open_tool_ui: flume::Sender<Box<dyn Send + Widget>>,
}

impl ToolsControllerBundle {
    pub fn new(tools: Vec<Tool>, open_tool_ui: flume::Sender<Box<dyn Send + Widget>>) -> Self {
        Self {
            tools,
            open_tool_ui,
        }
    }
}

impl Bundle for ToolsControllerBundle {
    fn mount(&self, entity: &mut flax::EntityBuilder) {
        let input = InputState::new();

        entity
            .set(
                tools_controller(),
                ToolsController::new(self.open_tool_ui.clone()),
            )
            .set(tools(), self.tools.clone())
            .set(input_state(), input)
            .set(current_tool(), Some(0));
    }
}

pub struct ToolsControllerPlugin;

impl Plugin for ToolsControllerPlugin {
    fn install(
        &self,
        _: &mut World,
        _: &AssetCache,
        _: &mut ivy_assets::stored::DynamicStore,
        schedules: &mut ScheduleSetBuilder,
    ) -> anyhow::Result<()> {
        schedules
            .per_tick_mut()
            .with_system(ToolsController::equip_tool_system());

        Ok(())
    }
}

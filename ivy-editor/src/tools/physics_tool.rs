use flax::{
    Entity, FetchExt, World, component,
    components::{child_of, name},
    system,
};
use futures::StreamExt;
use glam::Vec3;
use itertools::Itertools;
use ivy_core::{
    Bundle,
    plugin::{Plugin, PluginContext},
};
use ivy_physics::components::{angular_velocity, mass, velocity};
use ivy_ui::{
    streamed::StreamedUiExt,
    violet::core::{
        Scope, Widget,
        state::{StateExt, StateStreamRef},
        widget::{ScrollArea, StreamWidget, bold, card, col, label},
    },
};

use crate::plugin::{Selection, selection};

#[derive(Clone, Debug, PartialEq)]
struct PhysicsToolData {
    pub name: String,
    pub velocity: Vec3,
    pub angular_velocity: Vec3,
    pub mass: f32,
}

component! {
    physics_tool: PhysicsTool,
    physics_tool_data: Vec<PhysicsToolData>,
}

pub struct PhysicsTool {}

impl PhysicsTool {
    #[system(args(selection=selection().relation(child_of)), with_world)]
    pub fn update_system(
        self: &mut PhysicsTool,
        physics_tool_data: &mut Vec<PhysicsToolData>,
        selection: &Selection,
        world: &World,
    ) -> anyhow::Result<()> {
        physics_tool_data.clear();
        for entity in selection.entities() {
            let entity = world.entity(*entity)?;

            let velocity = entity.get_copy(velocity()).unwrap_or_default();
            let angular_velocity = entity.get_copy(angular_velocity()).unwrap_or_default();
            let mass = entity.get_copy(mass()).unwrap_or_default();

            physics_tool_data.push(PhysicsToolData {
                name: entity
                    .get_clone(name())
                    .ok()
                    .unwrap_or_else(|| "Unnamed".to_string()),
                velocity,
                angular_velocity,
                mass,
            });
        }

        Ok(())
    }
}

pub struct PhysicsToolBundle;

impl Bundle for PhysicsToolBundle {
    fn mount(&self, entity: &mut flax::EntityBuilder) {
        entity
            .set(physics_tool(), PhysicsTool {})
            .set(physics_tool_data(), Vec::new());
    }
}

pub struct PhysicsToolWidget {
    id: Entity,
}

impl PhysicsToolWidget {
    pub fn new(id: Entity) -> Self {
        Self { id }
    }
}

impl Widget for PhysicsToolWidget {
    fn mount(self, scope: &mut Scope<'_>) {
        let data = scope.stream_component(physics_tool_data(), self.id).dedup();

        let data = data.stream_ref(|data| {
            let data = data
                .iter()
                .map(|data| {
                    card(col((
                        bold(&data.name),
                        label(format!(
                            "Velocity: {:.1} {:.1} {:.1}",
                            data.velocity.x, data.velocity.y, data.velocity.z
                        )),
                        label(format!(
                            "Angular Velocity: {:.1} {:.1} {:.1}",
                            data.angular_velocity.x,
                            data.angular_velocity.y,
                            data.angular_velocity.z
                        )),
                        label(format!("Mass: {}kg", data.mass)),
                    )))
                })
                .collect_vec();

            ScrollArea::vertical(col(data))
        });

        StreamWidget::new(data).mount(scope);
    }
}

pub struct PhysicsToolPlugin;

impl Plugin for PhysicsToolPlugin {
    fn install(&self, ctx: &mut PluginContext) -> anyhow::Result<()> {
        ctx.schedules
            .per_tick_mut()
            .with_system(PhysicsTool::update_system());

        Ok(())
    }
}

use flax::{component, system, Component, FetchExt, Query, QueryBorrow, World};
use glam::{Mat4, Vec3};
use itertools::Itertools;
use ivy_assets::{stored::DynamicStore, AssetCache, Resource};
use ivy_core::{
    components::{gizmos, world_transform},
    gizmos::{Gizmos, LineGizmo, SphereGizmo},
    plugin::{Plugin, PluginContext},
    Bundle, Color, ColorExt,
};
use serde::{Deserialize, Serialize};

use super::movement_direction;

/// Automatically traverses the given path of waypoints
pub struct WaypointNavigator {
    waypoints: Vec<Vec3>,
}

impl WaypointNavigator {
    #[system(require_all)]
    fn update_system(
        self: &mut WaypointNavigator,
        world_transform: Mat4,
        movement_direction: &mut Vec3,
    ) {
        while let Some(waypoint) = self.waypoints.first() {
            let pos = world_transform.transform_point3(Vec3::ZERO);
            let rel = (*waypoint - pos).normalize();
            if rel.length() < 0.1 {
                self.waypoints.remove(0);
                continue;
            }

            *movement_direction = rel;
            break;
        }
    }

    pub fn set_waypoints(&mut self, waypoints: Vec<Vec3>) {
        self.waypoints = waypoints;
    }
}

#[system(with_query(Query::new(waypoint_navigator())))]
fn debug_waypoints_system(gizmos: &Gizmos, query: &mut QueryBorrow<Component<WaypointNavigator>>) {
    let mut gizmos = gizmos.begin_section("debug_waypoints");

    for waypoints in query {
        for (&a, &b) in waypoints.waypoints.iter().tuple_windows() {
            gizmos.draw(LineGizmo::from_points(a, b, 0.01, Color::blue()));
            gizmos.draw(SphereGizmo::new(b, 0.02, Color::blue()));
        }
    }
}

component! {
    pub waypoint_navigator: WaypointNavigator,
}

#[derive(Clone, Debug, Resource, Bundle, Serialize, Deserialize)]
pub struct WaypointNavigatorBundle {}

impl Bundle for WaypointNavigatorBundle {
    fn mount(&self, entity: &mut flax::EntityBuilder) {
        entity.set(
            waypoint_navigator(),
            WaypointNavigator {
                waypoints: Vec::new(),
            },
        );
    }
}

pub struct WaypointNavigatorPlugin;

impl Plugin for WaypointNavigatorPlugin {
    fn install(&self, ctx: PluginContext) -> anyhow::Result<()> {
        let PluginContext { world, assets, store, schedules } = ctx;
        schedules
            .fixed_mut()
            .with_system(WaypointNavigator::update_system())
            .with_system(debug_waypoints_system());

        Ok(())
    }
}

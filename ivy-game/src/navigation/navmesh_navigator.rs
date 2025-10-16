use constructive::{astar::Waypoint, navmesh::Navmesh};
use flax::{component, system, Component, FetchExt, Query, QueryBorrow};
use glam::{Mat4, Vec3};
use itertools::Itertools;
use ivy::{
    engine, gizmos,
    ivy_core::{
        gizmos::{Gizmos, Line, Sphere},
        plugin::Plugin,
        Bundle, Color, ColorExt,
    },
    position, world_transform,
};
use ivy_constructive::components::navmesh;

use crate::entity::BundleDesc;

use super::movement_direction;

pub struct NavmeshNavigator {
    waypoints: Vec<Waypoint>,
}

impl NavmeshNavigator {
    #[system(require_all, args(navmesh=navmesh().source(engine()), target_position=target_position()))]
    fn calculate_paths_system(
        self: &mut NavmeshNavigator,
        world_transform: Mat4,
        target_position: &Option<Vec3>,
        navmesh: &Navmesh,
    ) {
        let start = world_transform.transform_point3(Vec3::ZERO);

        if let Some(target_position) = target_position {
            let waypoints = navmesh.find_path(start, *target_position);

            let Some(waypoints) = waypoints else {
                return;
            };

            tracing::info!("new waypoints");
            self.waypoints = waypoints;
        } else {
            self.waypoints.clear();
        }
    }

    #[system]
    fn update_movement_system(
        self: &mut NavmeshNavigator,
        position: Vec3,
        movement_direction: &mut Vec3,
    ) {
        while let Some(p) = self.waypoints.last() {
            let rel = (p.point() - position).reject_from(Vec3::Y);

            tracing::info!(?rel);
            if rel.length() < 0.1 {
                self.waypoints.pop();
                continue;
            }

            *movement_direction = rel.normalize();
            break;
        }
    }
}

component! {
    pub navmesh_navigator: NavmeshNavigator,
    pub target_position: Option<Vec3>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct NavmeshNavigatorBundle {}

impl BundleDesc for NavmeshNavigatorBundle {
    type Bundle = Self;

    async fn load(&self, _: &ivy::ivy_assets::AssetCache) -> anyhow::Result<Self::Bundle>
    where
        Self: Sized,
    {
        Ok(self.clone())
    }
}

impl Bundle for NavmeshNavigatorBundle {
    fn mount(self, entity: &mut flax::EntityBuilder) {
        entity
            .set(
                navmesh_navigator(),
                NavmeshNavigator {
                    waypoints: Vec::new(),
                },
            )
            .set(target_position(), None);
    }
}

#[system(with_query(Query::new(navmesh_navigator())))]
fn debug_waypoints_system(gizmos: &Gizmos, query: &mut QueryBorrow<Component<NavmeshNavigator>>) {
    let mut gizmos = gizmos.begin_section("debug_navmesh_waypoints");

    for waypoints in query {
        for (&a, &b) in waypoints.waypoints.iter().tuple_windows() {
            gizmos.draw(Line::from_points(a.point(), b.point(), 0.01, Color::blue()));
            gizmos.draw(Sphere::new(b.point(), 0.02, Color::blue()));
        }
    }
}

register_bundle!(NavmeshNavigatorBundle, NavmeshNavigatorBundle);

pub struct NavmeshNavigatorPlugin;

impl Plugin for NavmeshNavigatorPlugin {
    fn install(&self, ctx: PluginContext) -> anyhow::Result<()> {
        let PluginContext { world, assets, store, schedules } = ctx;
        schedules
            .per_tick_mut()
            .with_system(NavmeshNavigator::calculate_paths_system())
            .with_system(NavmeshNavigator::update_movement_system())
            .with_system(debug_waypoints_system());

        Ok(())
    }
}

use anyhow::Context;
use glam::{Mat4, Vec3};
use itertools::Itertools;
use ivy_assets::{declare_resource, loadable::Loadable, AssetCache, Resource};
use ivy_core::Bundle;
use ivy_derive::Editable;
use ivy_gltf::GltfNodeDesc;
use ivy_physics::{
    components::collider_builder,
    rapier3d::{
        parry::shape::Cuboid,
        prelude::{Ball, ColliderBuilder, ConvexPolyhedron, SharedShape, TriMesh},
    },
    GltfConvexMeshDesc, GltfTriMeshDesc,
};

#[derive(Clone, Debug, Resource, Bundle)]
#[resource(derive = [Editable])]
pub struct GltfColliderBundle {
    #[resource(load)]
    shape: ColliderShape,
    #[resource_attr(editable(default = 1.0))]
    #[resource_attr(editable(range(0.0, 1.0)))]
    restitution: f32,
    #[resource_attr(editable(default = 1.0))]
    #[resource_attr(editable(range(0.0, 1.0)))]
    friction: f32,
    #[resource_attr(editable(default = 1.0))]
    #[resource_attr(editable(range(0.0, 1000.0)))]
    density: f32,
}

#[derive(Clone, Debug, Editable, serde::Serialize, serde::Deserialize)]
pub enum ColliderShapeDesc {
    Cuboid(#[editable(default = Vec3::ONE)] Vec3),
    Sphere(#[editable(default = 1.0)] f32),
    GltfTriMesh { node: GltfNodeDesc },
    GltfConvexMesh { node: GltfNodeDesc },
}

#[derive(Clone, Debug)]
pub enum ColliderShape {
    Cuboid(Cuboid),
    Sphere(Ball),
    TriMesh(TriMesh),
    ConvexMesh(ConvexPolyhedron),
}

impl ColliderShape {
    fn shared_shape(self) -> SharedShape {
        match self {
            ColliderShape::Cuboid(cuboid) => SharedShape::new(cuboid),
            ColliderShape::Sphere(ball) => SharedShape::new(ball),
            ColliderShape::TriMesh(tri_mesh) => SharedShape::new(tri_mesh),
            ColliderShape::ConvexMesh(convex_polyhedron) => SharedShape::new(convex_polyhedron),
        }
    }
}

declare_resource!(ColliderShape, ColliderShapeDesc);

impl Loadable for ColliderShapeDesc {
    type Output = ColliderShape;

    async fn load(&self, assets: &AssetCache) -> Result<ColliderShape, anyhow::Error> {
        let shape = match self {
            Self::GltfTriMesh { node } => {
                let node = node.load(assets).await?;
                let mesh = node.mesh().context("Missing mesh")?;

                ColliderShape::TriMesh(
                    GltfTriMeshDesc {
                        primitives: mesh.primitives().collect_vec(),
                        transform: Mat4::IDENTITY,
                    }
                    .create(assets)?,
                )
            }
            Self::GltfConvexMesh { node } => {
                let node = node.load(assets).await?;
                let mesh = node.mesh().context("Missing mesh")?;
                let primitive = mesh.primitives().next().unwrap();

                ColliderShape::ConvexMesh(
                    GltfConvexMeshDesc {
                        primitive,
                        transform: Mat4::IDENTITY,
                    }
                    .create(assets)?,
                )
            }
            Self::Cuboid(extent) => ColliderShape::Cuboid(Cuboid::new((*extent).into())),
            Self::Sphere(radius) => ColliderShape::Sphere(Ball::new(*radius)),
        };

        Ok(shape)
    }
}

impl Bundle for GltfColliderBundle {
    fn mount(&self, entity: &mut flax::EntityBuilder) {
        let builder = ColliderBuilder::new(self.shape.clone().shared_shape())
            .restitution(self.restitution)
            .density(self.density)
            .friction(self.friction);

        entity.set(collider_builder(), builder);
    }
}

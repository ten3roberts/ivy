use std::{convert::Infallible, f32::consts::PI};

use glam::{vec2, vec3, IVec3, Vec2, Vec3, Vec4};
use ivy_assets::AssetDesc;
use ivy_graphics::mesh::MeshData;
use ordered_float::Float;

use crate::mesh::Vertex;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UvSphereDesc {
    latitudes: u32,
    longitudes: u32,
}

impl Default for UvSphereDesc {
    fn default() -> Self {
        Self {
            latitudes: 32,
            longitudes: 24,
        }
    }
}

impl AssetDesc<MeshData> for UvSphereDesc {
    type Error = Infallible;

    fn load(
        &self,
        assets: &ivy_assets::AssetCache,
    ) -> Result<ivy_assets::Asset<MeshData>, Self::Error> {
        Ok(assets.insert(generate_uv_sphere(1.0, self.latitudes, self.longitudes)))
    }
}

fn generate_uv_sphere(radius: f32, latitudes: u32, longitudes: u32) -> MeshData {
    let latitudes = latitudes.max(3);
    let longitudes = longitudes.max(3);

    let mut positions = Vec::new();
    let mut tex_coords = Vec::new();
    let mut normals = Vec::new();

    let mut indices = Vec::new();

    let d_lat = PI / latitudes as f32;
    let d_long = 2.0 * PI / longitudes as f32;

    for i in 0..=latitudes {
        let phi = PI / 2.0 - i as f32 * d_lat;
        let xz = radius * phi.cos();
        let y = radius * phi.sin();

        for j in 0..=longitudes {
            let theta = j as f32 * d_long;

            let pos = vec3(xz * theta.cos(), y, xz * theta.sin());
            positions.push(pos);
            tex_coords.push(vec2(
                i as f32 / latitudes as f32,
                j as f32 / longitudes as f32,
            ));
            normals.push(pos.normalize());
        }
    }

    for i in 0..latitudes {
        let k1 = i * (longitudes + 1);
        let k2 = k1 + longitudes + 1;

        for j in 0..longitudes {
            let k1 = k1 + j;
            let k2 = k2 + j;
            if i != 0 {
                // indices.extend([k1, k2, k1 + 1]);
                indices.extend([k1 + 1, k2, k1]);
            }

            if i != latitudes - 1 {
                // indices.extend([k1 + 1, k2, k2 + 1])
                indices.extend([k2 + 1, k2, k1 + 1])
            }
        }
    }

    let mut mesh = MeshData::unskinned(indices, positions, tex_coords, normals);
    mesh.generate_tangents().unwrap();
    mesh
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PlaneDesc {
    normal: IVec3,
}

impl Default for PlaneDesc {
    fn default() -> Self {
        Self { normal: IVec3::Y }
    }
}

impl AssetDesc<MeshData> for PlaneDesc {
    type Error = Infallible;

    fn load(
        &self,
        assets: &ivy_assets::AssetCache,
    ) -> Result<ivy_assets::Asset<MeshData>, Self::Error> {
        Ok(assets.insert(generate_plane(1.0, self.normal.as_vec3())))
    }
}

pub fn generate_plane(halfextent: f32, normal: Vec3) -> MeshData {
    let tan = if normal.distance(Vec3::Y) < 0.001 {
        Vec3::Z
    } else {
        normal.cross(Vec3::Y)
    };

    let bitan = normal.cross(tan);

    let positions = [
        (-tan - bitan) * halfextent,
        (tan - bitan) * halfextent,
        (tan + bitan) * halfextent,
        (-tan + bitan) * halfextent,
    ];

    let tex_coords = [
        vec2(0.0, 1.0) * halfextent,
        vec2(1.0, 1.0) * halfextent,
        vec2(1.0, 0.0) * halfextent,
        vec2(0.0, 0.0) * halfextent,
    ];

    let normals = [normal; 4];

    let indices = [0, 1, 2, 2, 3, 0];

    let mut mesh = MeshData::unskinned(indices, positions, tex_coords, normals);
    mesh.generate_tangents().unwrap();
    mesh
}

pub fn generate_cube(halfextent: f32) -> MeshData {
    let sides = [
        (Vec3::X),
        (Vec3::Y),
        (Vec3::Z),
        (-Vec3::Z),
        (-Vec3::X),
        (-Vec3::Y),
    ];

    let mut positions = Vec::new();
    let mut tex_coords = Vec::new();
    let mut normals = Vec::new();
    let mut indices = Vec::new();

    for normal in sides {
        let tan = if normal.abs().distance(Vec3::Y) < 0.001 {
            Vec3::Z
        } else {
            normal.cross(Vec3::Y)
        };

        let bitan = normal.cross(tan);

        indices.extend([0, 1, 2, 2, 3, 0].map(|i| i + positions.len() as u32));

        positions.extend([
            (-tan - bitan) * halfextent + normal,
            (tan - bitan) * halfextent + normal,
            (tan + bitan) * halfextent + normal,
            (-tan + bitan) * halfextent + normal,
        ]);

        tex_coords.extend([
            vec2(0.0, 1.0) * halfextent,
            vec2(1.0, 1.0) * halfextent,
            vec2(1.0, 0.0) * halfextent,
            vec2(0.0, 0.0) * halfextent,
        ]);

        normals.extend([normal; 4]);
    }

    let mut mesh = MeshData::unskinned(indices, positions, tex_coords, normals);
    mesh.generate_tangents().unwrap();
    mesh
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct CubeDesc;

impl AssetDesc<MeshData> for CubeDesc {
    type Error = Infallible;

    fn load(
        &self,
        assets: &ivy_assets::AssetCache,
    ) -> Result<ivy_assets::Asset<MeshData>, Self::Error> {
        Ok(assets.insert(generate_cube(1.0)))
    }
}

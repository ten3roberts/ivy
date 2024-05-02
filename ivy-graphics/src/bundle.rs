use flax::EntityBuilder;
use glam::{Quat, Vec3};
use ivy_assets::Asset;
use ivy_base::{
    color, position, rotation, scale, visible, world_transform, Bundle, Color, Visible,
};
use tracing::field::Visit;

use crate::{
    components::{material, mesh},
    Material, Mesh,
};

/// Represents a bundle for anything that can be rendererd into the 3D world.
/// **Note**: This bundle is a superset of [`ivy_base::TransformBundle`] and the
/// transform bundle is thus superflous.
/// By default, the material is taken from the mesh if available. It is valid
/// for the material to be null, howver, I no materials exists in mesh the
/// object won't be rendererd.
pub struct RenderObjectBundle {
    pub pos: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
    pub color: Color,
    pub mesh: Asset<Mesh>,
    pub material: Option<Asset<Material>>,
}

impl Bundle for RenderObjectBundle {
    fn mount(self, entity: &mut EntityBuilder) {
        entity
            .set_default(world_transform())
            .set(visible(), Visible::Visible)
            .set(position(), self.pos)
            .set(rotation(), self.rotation)
            .set(scale(), self.scale)
            .set(color(), self.color)
            .set(mesh(), self.mesh)
            .set_opt(material(), self.material);
    }
}

// Implement manually since `Handle<Pass>` implements default and debug regardless of `Pass`
impl std::fmt::Debug for RenderObjectBundle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ObjectBundle")
            .field("pos", &self.pos)
            .field("rot", &self.rotation)
            .field("scale", &self.scale)
            .field("color", &self.color)
            // .field("pass", &self.pass)
            .field("mesh", &self.mesh.id())
            .field("material", &self.material)
            .finish()
    }
}

// impl Default for RenderObjectBundle {
//     fn default() -> Self {
//         Self {
//             visible: Visible::Visible,
//             pos: Default::default(),
//             rot: Default::default(),
//             scale: Default::default(),
//             color: Default::default(),
//             // pass: Default::default(),
//             mesh: Default::default(),
//             material: Default::default(),
//         }
//     }
// }

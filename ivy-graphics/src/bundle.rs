use flax::EntityBuilder;
use glam::{Quat, Vec3};
use ivy_base::{
    color, position, rotation, scale, visible, world_transform, Bundle, Color, Visible,
};
use ivy_resources::Handle;

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
    pub visible: Visible,
    pub pos: Vec3,
    pub rot: Quat,
    pub scale: Vec3,
    pub color: Color,
    pub mesh: Handle<Mesh>,
    pub material: Handle<Material>,
}

impl Bundle for RenderObjectBundle {
    fn mount(self, entity: &mut EntityBuilder) {
        entity
            .set_default(world_transform())
            .set(visible(), self.visible)
            .set(position(), self.pos)
            .set(rotation(), self.rot)
            .set(scale(), self.scale)
            .set(color(), self.color)
            .set(mesh(), self.mesh)
            .set(material(), self.material);
    }
}

// Implement manually since `Handle<Pass>` implements default and debug regardless of `Pass`
impl std::fmt::Debug for RenderObjectBundle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ObjectBundle")
            .field("visible", &self.visible)
            .field("pos", &self.pos)
            .field("rot", &self.rot)
            .field("scale", &self.scale)
            .field("color", &self.color)
            // .field("pass", &self.pass)
            .field("mesh", &self.mesh)
            .field("material", &self.material)
            .finish()
    }
}

impl Default for RenderObjectBundle {
    fn default() -> Self {
        Self {
            visible: Visible::Visible,
            pos: Default::default(),
            rot: Default::default(),
            scale: Default::default(),
            color: Default::default(),
            // pass: Default::default(),
            mesh: Default::default(),
            material: Default::default(),
        }
    }
}

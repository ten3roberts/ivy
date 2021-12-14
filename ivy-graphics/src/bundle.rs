use hecs::Bundle;
use ivy_base::{Color, Position, Rotation, Scale, Visible};
use ivy_resources::Handle;

use crate::{Material, Mesh};

/// Represents a bundle for anything that can be rendererd into the 3D world.
/// **Note**: This bundle is a superset of [`ivy_base::TransformBundle`] and the
/// transform bundle is thus superflous.
/// By default, the material is taken from the mesh if available. It is valid
/// for the material to be null, howver, I no materials exists in mesh the
/// object won't be rendererd.
#[derive(Bundle, Clone)]
pub struct ObjectBundle<Pass> {
    pub visible: Visible,
    pub pos: Position,
    pub rot: Rotation,
    pub scale: Scale,
    pub color: Color,
    pub pass: Handle<Pass>,
    pub mesh: Handle<Mesh>,
    pub material: Handle<Material>,
}

// Implement manually since `Handle<Pass>` implements default and debug regardless of `Pass`
impl<Pass> std::fmt::Debug for ObjectBundle<Pass> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ObjectBundle")
            .field("visible", &self.visible)
            .field("pos", &self.pos)
            .field("rot", &self.rot)
            .field("scale", &self.scale)
            .field("color", &self.color)
            .field("pass", &self.pass)
            .field("mesh", &self.mesh)
            .field("material", &self.material)
            .finish()
    }
}

impl<Pass> Default for ObjectBundle<Pass> {
    fn default() -> Self {
        Self {
            visible: Default::default(),
            pos: Default::default(),
            rot: Default::default(),
            scale: Default::default(),
            color: Default::default(),
            pass: Default::default(),
            mesh: Default::default(),
            material: Default::default(),
        }
    }
}

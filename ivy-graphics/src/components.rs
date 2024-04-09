use flax::Debuggable;
use ivy_resources::Handle;

use crate::{
    Animator, BoundingSphere, Camera, DepthAttachment, GpuCamera, LightRenderer, Material, Mesh,
    PointLight, Skin, SkinnedVertex,
};

flax::component! {
    pub mesh: Handle<Mesh>,
    pub skinned_mesh: Handle<Mesh<SkinnedVertex>>,
    pub skin: Handle<Skin>,
    pub material: Handle<Material>,

    /// Emission source for entity
    pub light: PointLight => [ Debuggable ],


    /// Drives the animation of an entity
    pub animator: Animator => [ Debuggable ],

    /// TODO: move to renderer node in rendergraph
    pub light_renderer: LightRenderer,

    pub camera: Camera => [ Debuggable ],
    pub gpu_camera: GpuCamera,

    pub bounding_sphere: BoundingSphere,

    pub depth_attachment: DepthAttachment,
}

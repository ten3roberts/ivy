use flax::Debuggable;
use ivy_assets::Asset;
use ivy_vulkan::Swapchain;
use ivy_window::Window;

use crate::{
    Animator, BoundingSphere, Camera, DepthAttachment, GpuCamera, LightRenderer, Material, Mesh,
    PointLight, Skin, SkinnedVertex,
};

flax::component! {
    pub mesh: Asset<Mesh>,
    pub skinned_mesh: Asset<Mesh<SkinnedVertex>>,
    pub skin: Asset<Skin>,
    pub material: Asset<Material>,

    /// Emission source for entity
    pub light_source: PointLight => [ Debuggable ],


    /// Drives the animation of an entity
    pub animator: Animator => [ Debuggable ],

    /// TODO: move to renderer node in rendergraph
    pub light_renderer: LightRenderer,

    pub camera: Camera => [ Debuggable ],
    pub gpu_camera: GpuCamera,

    pub bounding_sphere: BoundingSphere,

    pub depth_attachment: DepthAttachment,

    pub window: Window,
    pub swapchain: Swapchain,
}

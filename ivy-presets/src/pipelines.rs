//! Default shader passes to pass to the rendergraph for rendering different mesh passes.

use ivy_vulkan::Shader;

// TODO: investigate which of there are actually used
flax::component! {
    pub geometry_pass: Shader,
    pub fake_pass: Shader,
    pub transparent_pass: Shader,
    pub skinned_pass: Shader,
    pub ui_pass: Shader,
    pub text_pass: Shader,
    pub gizmo_pass: Shader,
    pub pbr_pass: Shader,
}

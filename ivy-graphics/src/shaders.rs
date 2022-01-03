use ivy_vulkan::ShaderModuleInfo;

/// Renders a fullscreen quad, has no input data
pub const FULLSCREEN_SHADER: ShaderModuleInfo =
    ShaderModuleInfo::from_const_bytes(include_bytes!("../../res/shaders/fullscreen.vert.spv"));

pub const DEFAULT_VERTEX_SHADER: ShaderModuleInfo =
    ShaderModuleInfo::from_const_bytes(include_bytes!("../../res/shaders/default.vert.spv"));

pub const DEFAULT_FRAGMENT_SHADER: ShaderModuleInfo =
    ShaderModuleInfo::from_const_bytes(include_bytes!("../../res/shaders/default.frag.spv"));

pub const GIZMO_VERTEX_SHADER: ShaderModuleInfo =
    ShaderModuleInfo::from_const_bytes(include_bytes!("../../res/shaders/gizmos.vert.spv"));
pub const GIZMO_FRAGMENT_SHADER: ShaderModuleInfo =
    ShaderModuleInfo::from_const_bytes(include_bytes!("../../res/shaders/gizmos.frag.spv"));

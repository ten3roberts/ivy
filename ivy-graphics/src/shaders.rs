use ivy_vulkan::ShaderModuleInfo;

/// Renders a fullscreen quad, has no input data
pub const FULLSCREEN_SHADER: ShaderModuleInfo = ShaderModuleInfo::from_const_bytes(include_bytes!(
    concat!(env!("OUT_DIR"), "/shaders/fullscreen.vert.spv")
));

pub const DEFAULT_VERTEX_SHADER: ShaderModuleInfo = ShaderModuleInfo::from_const_bytes(
    include_bytes!(concat!(env!("OUT_DIR"), "/shaders/default.vert.spv")),
);

pub const FORWARD_VERTEX_SHADER: ShaderModuleInfo = ShaderModuleInfo::from_const_bytes(
    include_bytes!(concat!(env!("OUT_DIR"), "/shaders/forward.vert.spv")),
);

pub const FORWARD_FRAGMENT_SHADER: ShaderModuleInfo = ShaderModuleInfo::from_const_bytes(
    include_bytes!(concat!(env!("OUT_DIR"), "/shaders/forward.frag.spv")),
);

pub const DEFAULT_FRAGMENT_SHADER: ShaderModuleInfo = ShaderModuleInfo::from_const_bytes(
    include_bytes!(concat!(env!("OUT_DIR"), "/shaders/default.frag.spv")),
);

pub const GLASS_FRAGMENT_SHADER: ShaderModuleInfo = ShaderModuleInfo::from_const_bytes(
    include_bytes!(concat!(env!("OUT_DIR"), "/shaders/glass.frag.spv")),
);
pub const GLASS_VERTEX_SHADER: ShaderModuleInfo = ShaderModuleInfo::from_const_bytes(
    include_bytes!(concat!(env!("OUT_DIR"), "/shaders/glass.vert.spv")),
);

pub const TRIPLANAR_FRAGMENT_SHADER: ShaderModuleInfo = ShaderModuleInfo::from_const_bytes(
    include_bytes!(concat!(env!("OUT_DIR"), "/shaders/triplanar.frag.spv")),
);

pub const SKINNED_VERTEX_SHADER: ShaderModuleInfo = ShaderModuleInfo::from_const_bytes(
    include_bytes!(concat!(env!("OUT_DIR"), "/shaders/skinned.vert.spv")),
);

pub const GIZMO_VERTEX_SHADER: ShaderModuleInfo = ShaderModuleInfo::from_const_bytes(
    include_bytes!(concat!(env!("OUT_DIR"), "/shaders/gizmos.vert.spv")),
);

pub const GIZMO_FRAGMENT_SHADER: ShaderModuleInfo = ShaderModuleInfo::from_const_bytes(
    include_bytes!(concat!(env!("OUT_DIR"), "/shaders/gizmos.frag.spv")),
);

// UI
pub const IMAGE_FRAGMENT_SHADER: ShaderModuleInfo = ShaderModuleInfo::from_const_bytes(
    include_bytes!(concat!(env!("OUT_DIR"), "/shaders/ui/image.frag.spv")),
);

pub const TEXT_FRAGMENT_SHADER: ShaderModuleInfo = ShaderModuleInfo::from_const_bytes(
    include_bytes!(concat!(env!("OUT_DIR"), "/shaders/ui/text.frag.spv")),
);

pub const IMAGE_VERTEX_SHADER: ShaderModuleInfo = ShaderModuleInfo::from_const_bytes(
    include_bytes!(concat!(env!("OUT_DIR"), "/shaders/ui/image.vert.spv")),
);

pub const TEXT_VERTEX_SHADER: ShaderModuleInfo = ShaderModuleInfo::from_const_bytes(
    include_bytes!(concat!(env!("OUT_DIR"), "/shaders/ui/text.vert.spv")),
);

// Pbr
/// Generic PBR lighting deferred lighting shader
pub const PBR_SHADER: ShaderModuleInfo = ShaderModuleInfo::from_const_bytes(include_bytes!(
    concat!(env!("OUT_DIR"), "/shaders/pbr_lighting.frag.spv")
));

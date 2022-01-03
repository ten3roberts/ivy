use ivy_vulkan::ShaderModuleInfo;

/// Generic PBR lighting deferred lighting shader
pub const PBR_SHADER: ShaderModuleInfo =
    ShaderModuleInfo::from_const_bytes(include_bytes!("../../res/shaders/pbr_lighting.frag.spv"));

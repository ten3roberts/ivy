use ivy_vulkan::ShaderModuleInfo;

pub const IMAGE_FRAGMENT_SHADER: ShaderModuleInfo =
    ShaderModuleInfo::from_const_bytes(include_bytes!("../../res/shaders/ui/image.frag.spv"));

pub const TEXT_FRAGMENT_SHADER: ShaderModuleInfo =
    ShaderModuleInfo::from_const_bytes(include_bytes!("../../res/shaders/ui/text.frag.spv"));

pub const IMAGE_VERTEX_SHADER: ShaderModuleInfo =
    ShaderModuleInfo::from_const_bytes(include_bytes!("../../res/shaders/ui/image.vert.spv"));

pub const TEXT_VERTEX_SHADER: ShaderModuleInfo =
    ShaderModuleInfo::from_const_bytes(include_bytes!("../../res/shaders/ui/text.vert.spv"));

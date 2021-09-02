use ash::vk;

pub trait VertexDesc {
    const BINDING_DESCRIPTIONS: &'static [vk::VertexInputBindingDescription];
    const ATTRIBUTE_DESCRIPTIONS: &'static [vk::VertexInputAttributeDescription];
}

impl VertexDesc for () {
    const BINDING_DESCRIPTIONS: &'static [vk::VertexInputBindingDescription] = &[];

    const ATTRIBUTE_DESCRIPTIONS: &'static [vk::VertexInputAttributeDescription] = &[];
}

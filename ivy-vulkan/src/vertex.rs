use ash::vk;

pub trait VertexDesc {
    const BINDING_DESCRIPTION: vk::VertexInputBindingDescription;
    const ATTRIBUTE_DESCRIPTIONS: &'static [vk::VertexInputAttributeDescription];
}

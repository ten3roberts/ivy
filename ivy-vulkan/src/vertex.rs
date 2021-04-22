use ash::vk;

pub trait VertexDesc {
    fn binding_description() -> vk::VertexInputBindingDescription;
    fn attribute_descriptions() -> &'static [vk::VertexInputAttributeDescription];
}

use ash::vk;
use glam::Vec3;

pub trait VertexDesc {
    const BINDING_DESCRIPTIONS: &'static [vk::VertexInputBindingDescription];
    const ATTRIBUTE_DESCRIPTIONS: &'static [vk::VertexInputAttributeDescription];
    fn pos(&self) -> Vec3;
}

impl VertexDesc for () {
    const BINDING_DESCRIPTIONS: &'static [vk::VertexInputBindingDescription] = &[];

    const ATTRIBUTE_DESCRIPTIONS: &'static [vk::VertexInputAttributeDescription] = &[];

    fn pos(&self) -> Vec3 {
        Vec3::ZERO
    }
}

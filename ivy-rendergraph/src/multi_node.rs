// use crate::Node;

// /// A rendergraph node combinding multiple nodes into a single one. This is useful to combine two
// /// separate side effects into a single subpass.
// pub struct MultiNode {
//     nodes: Vec<Box<dyn Node>>,
//     color_attachments: Vec<AttachmentInfo>,
//     input_attachments: Vec<Handle<Texture>>,
//     read_attachments: Vec<Handle<Texture>>,
// }

// impl MultiNode {
//     pub fn new(nodes: Vec<Box<dyn Node>>) -> Self { Self { nodes } }
// }

// impl Node for MultiNode {
//     fn color_attachments(&self) -> &[crate::AttachmentInfo] {

//     }

//     fn read_attachments(&self) -> &[ivy_resources::Handle<ivy_vulkan::Texture>] {
//         todo!()
//     }

//     fn input_attachments(&self) -> &[ivy_resources::Handle<ivy_vulkan::Texture>] {
//         todo!()
//     }

//     fn depth_attachment(&self) -> Option<&crate::AttachmentInfo> {
//         todo!()
//     }

//     fn clear_values(&self) -> &[ivy_vulkan::vk::ClearValue] {
//         todo!()
//     }

//     fn node_kind(&self) -> crate::NodeKind {
//         todo!()
//     }

//     fn execute(
//         &mut self,
//         world: &mut hecs::World,
//         cmd: &ivy_vulkan::commands::CommandBuffer,
//         current_frame: usize,
//         resources: &ivy_resources::Resources,
//     ) -> anyhow::Result<()> {
//         todo!()
//     }
// }

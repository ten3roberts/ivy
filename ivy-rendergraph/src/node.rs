use anyhow::Context;
use flax::{Component, World};
use ivy_assets::{Asset, AssetCache};
use ivy_graphics::Renderer;
use ivy_vulkan::{
    commands::CommandBuffer,
    vk::{Buffer, ClearValue},
    ClearValueExt, ImageLayout, LoadOp, PassInfo, Shader, StoreOp, Texture,
};
use std::any::type_name;

/// Represents a node in the renderpass.
pub trait Node: 'static + Send + Sync {
    /// Returns the color attachments for this node. Should not be execution heavy function
    fn color_attachments(&self) -> &[AttachmentInfo] {
        &[]
    }

    fn output_attachments(&self) -> &[Asset<Texture>] {
        &[]
    }
    /// Returns the read attachments for this node. Should not be execution heavy function
    fn read_attachments(&self) -> &[Asset<Texture>] {
        &[]
    }
    /// Partially sampled input attachments. Read from the same pixel coord we write to
    fn input_attachments(&self) -> &[Asset<Texture>] {
        &[]
    }
    /// Returns the optional depth attachment for this node. Should not be execution heavy function
    fn depth_attachment(&self) -> Option<&AttachmentInfo> {
        None
    }

    fn buffer_reads(&self) -> &[Buffer] {
        &[]
    }

    fn buffer_writes(&self) -> &[Buffer] {
        &[]
    }

    /// Returns the clear values to initiate this renderpass
    fn clear_values(&self) -> &[ClearValue] {
        &[]
    }

    fn node_kind(&self) -> NodeKind;

    // Optional name, can be empty string
    fn debug_name(&self) -> &'static str;

    /// Execute this node inside a compatible renderpass
    fn execute(
        &mut self,
        world: &mut World,
        assets: &AssetCache,
        cmd: &CommandBuffer,
        pass_info: &PassInfo,
        current_frame: usize,
    ) -> anyhow::Result<()>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    // A graphics rendering node. Renderpass and framebuffer will automatically be created.
    Graphics,
    // execution
    // A node that will be executed on the transfer queue. Appropriate pipeline barriers will
    // be inserted
    Transfer,
    // Compute,
}

#[derive(Clone)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub struct AttachmentInfo {
    // TODO, derive from edges
    pub store_op: StoreOp,
    pub load_op: LoadOp,
    pub initial_layout: ImageLayout,
    pub final_layout: ImageLayout,
    pub resource: Asset<Texture>,
    pub clear_value: ClearValue,
}

// impl Default for AttachmentInfo {
//     fn default() -> Self {
//         Self {
//             store_op: StoreOp::STORE,
//             load_op: LoadOp::DONT_CARE,
//             initial_layout: ImageLayout::UNDEFINED,
//             final_layout: ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
//             resource: todo!(),
//             clear_value: ClearValue::default(),
//         }
//     }
// }

impl AttachmentInfo {
    pub fn color(resource: Asset<Texture>) -> Self {
        Self {
            store_op: StoreOp::STORE,
            load_op: LoadOp::CLEAR,
            initial_layout: ImageLayout::UNDEFINED,
            final_layout: ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            clear_value: ClearValue::color(0.0, 0.0, 0.0, 1.0),
            resource,
        }
    }

    pub fn depth_discard(resource: Asset<Texture>) -> Self {
        Self {
            store_op: StoreOp::DONT_CARE,
            load_op: LoadOp::CLEAR,
            initial_layout: ImageLayout::UNDEFINED,
            final_layout: ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
            clear_value: ClearValue::depth_stencil(1.0, 0),
            resource,
        }
    }
    pub fn depth_store(resource: Asset<Texture>) -> Self {
        Self {
            store_op: StoreOp::STORE,
            load_op: LoadOp::CLEAR,
            initial_layout: ImageLayout::UNDEFINED,
            final_layout: ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
            clear_value: ClearValue::depth_stencil(1.0, 0),
            resource,
        }
    }
}

/// Simple node for rendering a pass in the rendergraph using the provided renderer
pub struct RenderNode<T> {
    renderer: T,
    shaderpass: Component<Shader>,
}

impl<T> RenderNode<T> {
    pub fn new(renderer: T, shaderpass: Component<Shader>) -> Self {
        Self {
            renderer,
            shaderpass,
        }
    }
}

impl<T> Node for RenderNode<T>
where
    T: 'static + Renderer + Send + Sync,
{
    fn node_kind(&self) -> NodeKind {
        NodeKind::Graphics
    }

    fn execute(
        &mut self,
        world: &mut World,
        assets: &AssetCache,
        cmd: &CommandBuffer,
        pass_info: &PassInfo,
        current_frame: usize,
    ) -> anyhow::Result<()> {
        self.renderer
            .draw(
                world,
                assets,
                cmd,
                &[],
                pass_info,
                &[],
                current_frame,
                self.shaderpass,
            )
            .with_context(|| format!("Failed to draw using {:?}", type_name::<T>()))
    }

    fn debug_name(&self) -> &'static str {
        std::any::type_name::<RenderNode<T>>()
    }
}

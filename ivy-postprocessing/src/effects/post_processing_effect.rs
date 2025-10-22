use ivy_wgpu::{
    rendergraph::{RenderGraph, TextureHandle},
    Gpu,
};

/// Trait for post-processing effects that can be added to the render graph
pub trait PostProcessingEffect {
    fn add_to_graph(
        &self,
        gpu: &Gpu,
        render_graph: &mut RenderGraph,
        input: TextureHandle,
        output: TextureHandle,
        resolved_depth_texture: Option<TextureHandle>,
    );
}
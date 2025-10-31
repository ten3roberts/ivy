use crate::effects::PostProcessingEffect;
use ivy_wgpu::{
    rendergraph::{RenderGraph, TextureHandle},
    Gpu,
};

#[derive(Debug, Clone)]
pub struct BloomConfig {
    pub filter_radius: f32,
    pub layers: u32,
}

impl Default for BloomConfig {
    fn default() -> Self {
        Self {
            filter_radius: 0.001,
            layers: 4,
        }
    }
}

impl PostProcessingEffect for BloomConfig {
    fn add_to_graph(
        &self,
        gpu: &Gpu,
        render_graph: &mut RenderGraph,
        input: TextureHandle,
        output: TextureHandle,
        _resolved_depth_texture: Option<TextureHandle>,
    ) {
        render_graph.add_node(crate::bloom::BloomNode::new(
            gpu,
            input,
            output,
            self.layers,
            self.filter_radius,
        ));
    }
}

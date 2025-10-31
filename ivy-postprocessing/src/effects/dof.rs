use crate::effects::PostProcessingEffect;
use ivy_wgpu::{
    rendergraph::{RenderGraph, TextureHandle},
    Gpu,
};

/// Configuration for the Depth of Field post-processing effect.
#[derive(Debug, Clone)]
pub struct DofConfig {
    /// The radius of the blur filter.
    pub filter_radius: f32,
    /// Number of layers for the effect.
    pub layers: u32,
    /// Distance at which objects are in focus.
    pub focus_distance: f32,
    /// Range around focus distance where objects are in focus.
    pub focus_range: f32,
}

/// Default configuration values for Depth of Field.
impl Default for DofConfig {
    fn default() -> Self {
        Self {
            filter_radius: 0.0001,
            layers: 4,
            focus_distance: 15.0,
            focus_range: 50.0,
        }
    }
}

/// Implementation of the PostProcessingEffect trait for DofConfig.
impl PostProcessingEffect for DofConfig {
    fn add_to_graph(
        &self,
        gpu: &Gpu,
        render_graph: &mut RenderGraph,
        input: TextureHandle,
        output: TextureHandle,
        resolved_depth_texture: Option<TextureHandle>,
    ) {
        let depth_texture = resolved_depth_texture.expect("DoF requires resolved depth texture");
        render_graph.add_node(crate::dof::DepthOfFieldNode::new(
            gpu,
            input,
            depth_texture,
            output,
            self.layers,
            self.filter_radius,
            self.focus_distance,
            self.focus_range,
        ));
    }
}

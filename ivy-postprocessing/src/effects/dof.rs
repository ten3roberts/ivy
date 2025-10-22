use crate::effects::PostProcessingEffect;
use ivy_wgpu::{
    rendergraph::{RenderGraph, TextureHandle},
    Gpu,
};

#[derive(Debug, Clone)]
pub struct DofConfig {
    pub filter_radius: f32,
    pub layers: u32,
    pub focus_distance: f32,
    pub focus_range: f32,
    pub near: f32,
    pub far: f32,
}

impl Default for DofConfig {
    fn default() -> Self {
        Self {
            filter_radius: 0.0001,
            layers: 1,
            focus_distance: 15.0,
            focus_range: 100.0,
            near: 0.1,
            far: 1000.0,
        }
    }
}

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
            self.near,
            self.far,
        ));
    }
}
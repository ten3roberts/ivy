use std::sync::Arc;

use ivy_rendergraph::RenderGraph;
use ivy_vulkan::VulkanContext;

pub const FRAMES_IN_FLIGHT: usize = 2;

#[test]
fn basic() -> Result<(), Box<dyn std::error::Error>> {
    let context = Arc::new(VulkanContext::new_offscreen()?);
    let graph = RenderGraph::new(context.clone(), FRAMES_IN_FLIGHT);

    Ok(())
}

use std::sync::Arc;

use parking_lot::Mutex;

use crate::RenderGraph;

flax::component! {
    pub render_graph: Arc<Mutex<RenderGraph>>,
}

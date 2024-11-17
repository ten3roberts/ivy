use ivy_wgpu::rendergraph::TextureHandle;
use violet::{
    core::{
        components::{color, draw_shape},
        shape,
        style::{SizeExt, WidgetSize},
        Scope, Widget,
    },
    palette::Srgba,
    wgpu::components::texture_handle,
};

use crate::components::texture_dependency;

pub struct RendergraphImage {
    size: WidgetSize,
    handle: TextureHandle,
}

impl RendergraphImage {
    pub fn new(handle: TextureHandle) -> Self {
        Self {
            size: Default::default(),
            handle,
        }
    }
}

impl Widget for RendergraphImage {
    fn mount(self, scope: &mut Scope) {
        self.size.mount(scope);
        scope
            .set(color(), Srgba::new(1.0, 1.0, 1.0, 1.0))
            .set(draw_shape(shape::shape_rectangle()), ())
            .set(texture_handle(), None) // Filled in by render node when available
            .set(texture_dependency(), self.handle);
    }
}

impl SizeExt for RendergraphImage {
    fn size_mut(&mut self) -> &mut WidgetSize {
        &mut self.size
    }
}

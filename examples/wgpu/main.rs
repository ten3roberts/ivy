use color_eyre::{eyre::bail, owo_colors::OwoColorize, Result};
use tracing::info;
use tracing_subscriber::{prelude::*, Registry};
use tracing_tree::HierarchicalLayer;
use winit::{
    dpi::PhysicalSize,
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

#[tokio::main]
async fn main() -> Result<()> {
    let subscriber = Registry::default().with(HierarchicalLayer::new(2));
    tracing::subscriber::set_global_default(subscriber)?;

    let events = EventLoop::new();
    let window = WindowBuilder::new()
        .with_inner_size(PhysicalSize::new(800, 600))
        .with_decorations(true)
        .build(&events)?;

    info!("Opening window");

    events.run(move |event, _, ctl| match event {
        Event::WindowEvent {
            ref event,
            window_id,
        } if window_id == window.id() => match event {
            WindowEvent::CloseRequested => {
                *ctl = ControlFlow::Exit;
            }
            _ => {}
        },
        _ => {}
    });
}

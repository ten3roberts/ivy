use flax::{Entity, Query};
use glam::Vec2;
use ivy_core::components::{engine, request_capture_mouse};
use ivy_input::components::input_state;
use ivy_ui::{
    components::on_input_event,
    image::RendergraphImage,
    screens::{Screen, ScreenLifetimeToken, ScreenStack, ScreenState},
    streamed::{streamed_state, StreamedState, StreamedUiExt},
    violet::core::{
        components::rect,
        input::{interactive, keep_focus},
        style::{default_corner_radius, SizeExt},
        unit::Unit,
        widget::{
            interactive::overlay::{overlay_state, OverlayStack, OverlayState},
            maximized, row, Stack,
        },
        Scope, Widget,
    },
};
use ivy_wgpu::rendergraph::TextureHandle;

use crate::{scene_world, viewport_provider::scene_viewport_state};

pub struct OpenViewport {
    pub scene: Entity,
    pub view: TextureHandle,
    pub on_size: Box<dyn Send + Sync + FnMut(Vec2)>,
    pub streamed: StreamedState,
    pub screen_state: ScreenState,
}

pub enum SceneViewCommand {
    OpenViewport(OpenViewport),
    // TODO: pull into open command result
    CloseViewport { scene: Entity },
}

/// Shows the active scene from available [`SceneViewportProvider`] context
pub struct SceneView {}

impl SceneView {
    pub fn new() -> Self {
        Self {}
    }
}

impl Widget for SceneView {
    fn mount(self, scope: &mut Scope<'_>) {
        let scene_views = scope
            .get_context(scene_viewport_state())
            .register_listener();

        let mut current_viewport: Option<(Entity, Entity)> = None;

        let on_command = move |scope: &mut Scope<'_>, item: SceneViewCommand| match item {
            SceneViewCommand::OpenViewport(OpenViewport {
                scene,
                view,
                on_size,
                streamed: streamed_tx,
                screen_state,
            }) => {
                let id = scope.attach(SceneViewport {
                    streamed_state: streamed_tx,
                    scene,
                    on_size,
                    view,
                    screen_state,
                });

                current_viewport = Some((id, scene));
            }
            SceneViewCommand::CloseViewport { scene } => {
                if let Some((id, current_scene)) = current_viewport {
                    if current_scene == scene {
                        scope.detach(id);
                        current_viewport = None;
                    }
                }
            }
        };

        scope.spawn_stream(scene_views.into_stream(), on_command);

        Stack::new(()).with_maximize(Vec2::ONE).mount(scope)
    }
}

/// A viewport showing a scene, capturing input and forwarding it to the scene's input state
pub struct SceneViewport {
    streamed_state: StreamedState,
    scene: Entity,
    on_size: Box<dyn Send + Sync + FnMut(Vec2)>,
    view: TextureHandle,
    screen_state: ScreenState,
}

impl SceneViewport {
    pub fn new(viewport: OpenViewport) -> Self {
        Self {
            streamed_state: viewport.streamed,
            scene: viewport.scene,
            on_size: viewport.on_size,
            view: viewport.view,
            screen_state: viewport.screen_state,
        }
    }
}

impl Widget for SceneViewport {
    fn mount(self, scope: &mut Scope<'_>) {
        scope.set_context(streamed_state(), self.streamed_state);

        // TODO: maybe a better way for this
        let screens = ScreenStack::new(self.screen_state);
        screens
            .state()
            .open_below(SceneInputCapture::new(self.scene));

        let overlays = OverlayState::new();

        scope.set_context(overlay_state(), overlays.clone());

        Stack::new((
            SceneImage::new(self.view, self.on_size),
            row(screens).with_contain_margins(true),
            OverlayStack::from_state(overlays),
        ))
        .mount(scope)
    }
}

pub struct SceneInputCapture {
    world_id: Entity,
}

impl SceneInputCapture {
    pub fn new(world_id: Entity) -> Self {
        Self { world_id }
    }
}

impl Screen for SceneInputCapture {
    fn create(self, scope: &mut Scope<'_>, _: ScreenLifetimeToken) {
        let capture_mouse = scope.stream_component(request_capture_mouse(), engine());

        scope.spawn_stream(capture_mouse.into_stream(), |scope, value| {
            tracing::info!("Setting capture mouse to {value}");
            scope.set(request_capture_mouse(), value);
        });

        scope
            .set(
                on_input_event(),
                Box::new(move |_, engine_world, _, event| {
                    let scene_world = engine_world.get_mut(self.world_id, scene_world())?;

                    Query::new(input_state().as_mut())
                        .borrow(&scene_world)
                        .for_each(|v| v.apply(event));

                    Ok(())
                }),
            )
            .set_default(keep_focus())
            .set_default(interactive());

        maximized(()).mount(scope)
    }

    fn block_lower_input(&self) -> bool {
        true
    }
}

pub struct SceneImage {
    view: TextureHandle,
    on_size: Box<dyn Send + Sync + FnMut(Vec2)>,
}

impl SceneImage {
    pub fn new(view: TextureHandle, on_size: Box<dyn Send + Sync + FnMut(Vec2)>) -> Self {
        Self { view, on_size }
    }
}

impl Widget for SceneImage {
    fn mount(mut self, scope: &mut Scope<'_>) {
        scope.monitor(rect(), move |rect| {
            if let Some(rect) = rect {
                (self.on_size)(rect.size())
            }
        });

        RendergraphImage::new(self.view)
            .with_corner_radius(default_corner_radius())
            .with_maximize(Vec2::ONE)
            .with_min_size(Unit::px2(100.0, 100.0))
            .mount(scope);
    }
}

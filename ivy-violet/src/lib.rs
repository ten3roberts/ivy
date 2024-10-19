use std::{cell::RefCell, ops::Deref, rc::Rc};

use anyhow::Context;
use flax::World;
use ivy_assets::AssetCache;
use ivy_core::{app::TickEvent, profiling::profile_function, Layer};
use ivy_input::types::InputEvent;
use ivy_wgpu::{
    events::{ApplicationReady, ResizedEvent},
    rendergraph::{Dependency, Node, TextureHandle},
    types::PhysicalSize,
    Gpu,
};
use violet::{
    core::{
        components::{rect, size},
        Widget,
    },
    glam::{vec2, Mat4, Vec2},
    wgpu::{
        app::AppInstance,
        renderer::{MainRenderer, MainRendererConfig, RendererContext},
    },
};
use wgpu::TextureUsages;
use winit::dpi::LogicalSize;

pub type SharedUiInstance = Rc<RefCell<AppInstance>>;

pub struct UILayer {
    instance: Rc<RefCell<AppInstance>>,
}

impl UILayer {
    pub fn new(root: impl Widget) -> Self {
        let instance = Rc::new(RefCell::new(AppInstance::new(root)));

        Self { instance }
    }

    fn on_ready(&mut self, _: &mut World, _: &mut AssetCache) -> anyhow::Result<()> {
        Ok(())
    }

    fn on_input_event(
        &mut self,
        _: &mut World,
        _: &mut AssetCache,
        event: &InputEvent,
    ) -> anyhow::Result<()> {
        profile_function!();
        tracing::info!(?event, "ui input");
        let instance = &mut *self.instance.deref().borrow_mut();

        // todo: modifiers changed
        match event {
            InputEvent::Keyboard(keyboard_input) => {
                instance.input_state.on_keyboard_input(
                    &mut instance.frame,
                    keyboard_input.key.clone(),
                    keyboard_input.state,
                    None,
                );
            }
            InputEvent::Scroll(scroll_motion) => instance
                .input_state
                .on_scroll(&mut instance.frame, scroll_motion.delta),
            InputEvent::MouseButton(mouse_input) => instance.input_state.on_mouse_input(
                &mut instance.frame,
                mouse_input.state,
                mouse_input.button,
            ),
            InputEvent::CursorMoved(cursor_moved) => instance.input_state.on_cursor_move(
                &mut instance.frame,
                vec2(
                    cursor_moved.absolute_position.x,
                    cursor_moved.absolute_position.y,
                ),
            ),
            InputEvent::CursorDelta(_) => {}
            InputEvent::CursorLeft => {}
            InputEvent::CursorEntered => {}
            InputEvent::Focus(_) => {}
        }

        Ok(())
    }

    fn on_tick(&mut self, _: &mut World, _: &mut AssetCache) -> anyhow::Result<()> {
        profile_function!();

        let mut instance = self.instance.deref().borrow_mut();

        instance.update();
        Ok(())
    }

    fn on_resized(
        &mut self,
        _: &mut World,
        _: &mut AssetCache,
        event: &ResizedEvent,
    ) -> anyhow::Result<()> {
        let mut instance = self.instance.deref().borrow_mut();

        instance.on_resize(event.physical_size);
        Ok(())
    }

    /// Now be careful with this one, alright?
    pub fn instance(&self) -> &SharedUiInstance {
        &self.instance
    }
}

impl Layer for UILayer {
    fn register(
        &mut self,
        _: &mut World,
        _: &AssetCache,
        mut events: ivy_core::layer::events::EventRegisterContext<Self>,
    ) -> anyhow::Result<()>
    where
        Self: Sized,
    {
        events.subscribe(|this, world, assets, _: &ApplicationReady| this.on_ready(world, assets));

        events.subscribe(|this, world, assets, event: &InputEvent| {
            this.on_input_event(world, assets, event)
        });

        events.subscribe(|this, world, assets, _: &TickEvent| this.on_tick(world, assets));

        events.subscribe(|this, world, assets, event: &ResizedEvent| {
            this.on_resized(world, assets, event)
        });

        Ok(())
    }
}

pub struct UiRenderNode {
    instance: SharedUiInstance,
    renderer: Option<MainRenderer>,
    ctx: RendererContext,
    target: TextureHandle,
}

impl UiRenderNode {
    pub fn new(gpu: &Gpu, ui_instance: SharedUiInstance, target: TextureHandle) -> Self {
        Self {
            instance: ui_instance,
            renderer: None,
            ctx: RendererContext::new(violet::wgpu::Gpu {
                adapter: gpu.adapter.clone(),
                device: gpu.device.clone(),
                queue: gpu.queue.clone(),
            }),
            target,
        }
    }
}

impl Node for UiRenderNode {
    fn draw(&mut self, ctx: ivy_wgpu::rendergraph::NodeExecutionContext) -> anyhow::Result<()> {
        let target = ctx.get_texture(self.target);
        let target_view = target.create_view(&Default::default());

        let instance = &mut *self.instance.deref().borrow_mut();

        let root = instance.frame.world_mut().entity(instance.root)?;

        let size = root
            .get_copy(rect())
            .context("missing size for canvas")?
            .size();
        tracing::info!(?size, "ui size");
        self.ctx.globals.projview = Mat4::orthographic_lh(0.0, size.x, size.y, 0.0, 0.0, 1000.0);
        self.ctx
            .globals_buffer
            .write(&self.ctx.gpu.queue, 0, &[self.ctx.globals]);

        let renderer = self.renderer.get_or_insert_with(|| {
            let text_system = instance.text_system().clone();
            let layout_changes_rx = instance.layout_changes_rx().clone();
            let root = instance.root();
            let frame = &mut instance.frame;

            MainRenderer::new(
                frame,
                &mut self.ctx,
                root,
                text_system,
                target.format(),
                layout_changes_rx,
                MainRendererConfig { debug_mode: false },
            )
        });

        renderer.resize(
            &self.ctx,
            PhysicalSize {
                width: target.size().width,
                height: target.size().height,
            },
            1.0,
        );

        renderer.update(&mut self.ctx, &mut instance.frame)?;
        renderer.draw(
            &mut self.ctx,
            &mut instance.frame,
            ctx.encoder,
            &target_view,
            false,
        )?;

        Ok(())
    }

    fn on_resource_changed(&mut self, _resource: ivy_wgpu::rendergraph::ResourceHandle) {
        todo!()
    }

    fn read_dependencies(&self) -> Vec<ivy_wgpu::rendergraph::Dependency> {
        vec![Dependency::texture(
            self.target,
            TextureUsages::RENDER_ATTACHMENT,
        )]
    }

    fn write_dependencies(&self) -> Vec<ivy_wgpu::rendergraph::Dependency> {
        vec![]
    }
}

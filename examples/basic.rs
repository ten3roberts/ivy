use flax::{Entity, World};
use glam::{vec3, Mat4, Quat, Vec3};
use ivy_assets::AssetCache;
use ivy_base::{
    app::{InitEvent, TickEvent},
    layer::events::EventRegisterContext,
    main_camera,
    palette::angle::FromAngle,
    rotation, App, EngineLayer, EntityBuilderExt, Layer, TransformBundle,
};
use ivy_wgpu::{
    components::projection_matrix,
    driver::WinitDriver,
    events::KeyboardInput,
    layer::GraphicsLayer,
    material::MaterialDesc,
    mesh::{MeshData, MeshDesc},
    renderer::RenderObjectBundle,
    shader::ShaderDesc,
    texture::TextureDesc,
};
use tracing_subscriber::{layer::SubscriberExt, registry, util::SubscriberInitExt, EnvFilter};
use tracing_tree::HierarchicalLayer;

pub fn main() -> anyhow::Result<()> {
    registry()
        .with(EnvFilter::from_default_env())
        .with(HierarchicalLayer::default().with_indent_lines(true))
        .init();

    if let Err(err) = App::builder()
        .with_driver(WinitDriver::new())
        .with_layer(EngineLayer::new())
        .with_layer(GraphicsLayer::new())
        .with_layer(LogicLayer::new())
        .run()
    {
        tracing::error!("{err:?}");
        Err(err)
    } else {
        Ok(())
    }
}

pub struct LogicLayer {
    entity: Option<Entity>,
}

impl Default for LogicLayer {
    fn default() -> Self {
        Self::new()
    }
}

impl LogicLayer {
    pub fn new() -> Self {
        Self { entity: None }
    }

    fn setup_objects(&mut self, world: &mut World, assets: &AssetCache) -> anyhow::Result<()> {
        let shader = assets.insert(ShaderDesc::new(
            "diffuse",
            include_str!("../assets/shaders/diffuse.wgsl"),
        ));

        let mesh = assets.insert(MeshDesc::content(assets.insert(MeshData::quad())));

        let material = assets.insert(MaterialDesc::new(TextureDesc::path(
            "assets/textures/statue.jpg",
        )));

        let material2 = assets.insert(MaterialDesc::new(TextureDesc::path(
            "assets/textures/grid.png",
        )));

        Entity::builder()
            .mount(RenderObjectBundle::new(
                mesh.clone(),
                material.clone(),
                shader.clone(),
            ))
            .mount(TransformBundle::new(Vec3::Z, Quat::IDENTITY, Vec3::ONE))
            .spawn(world);

        let entity = Entity::builder()
            .mount(RenderObjectBundle::new(mesh, material2, shader))
            .mount(TransformBundle::new(
                vec3(1.0, 0.0, 1.0),
                Quat::from_euler(glam::EulerRot::ZYX, 1.0, 0.0, 0.0),
                Vec3::ONE,
            ))
            .spawn(world);

        self.entity = Some(entity);

        Ok(())
    }
}

impl Layer for LogicLayer {
    fn register(
        &mut self,
        world: &mut World,
        assets: &AssetCache,
        mut events: EventRegisterContext<Self>,
    ) -> anyhow::Result<()> {
        events.subscribe(|this, world, assets, InitEvent| this.setup_objects(world, assets));

        let start_time = std::time::Instant::now();
        events.subscribe(move |this, world, _, _: &TickEvent| {
            let t = start_time.elapsed().as_secs_f32();
            if let Some(entity) = this.entity {
                world
                    .set(entity, rotation(), Quat::from_axis_angle(Vec3::Z, t))
                    .unwrap();
            }
            Ok(())
        });

        Entity::builder()
            .mount(TransformBundle::new(Vec3::ZERO, Quat::IDENTITY, Vec3::ONE))
            .set(main_camera(), ())
            .set(
                projection_matrix(),
                Mat4::orthographic_lh(-5.0, 5.0, -5.0, 5.0, 0.1, 1000.0),
            )
            .spawn(world);

        Ok(())
    }
}

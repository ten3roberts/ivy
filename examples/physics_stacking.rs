use flax::{Entity, Query, World};
use glam::{vec3, EulerRot, Mat4, Quat, Vec3};
use ivy_assets::{fs::AssetPath, AssetCache};
use ivy_core::{
    app::PostInitEvent,
    layer::events::EventRegisterContext,
    palette::{Srgb, Srgba},
    profiling::ProfilingLayer,
    update_layer::{FixedTimeStep, ScheduledLayer},
    App, Color, ColorExt, EngineLayer, EntityBuilderExt, Layer,
};
use ivy_engine::{is_static, main_camera, rotation, scale, RigidBodyBundle, TransformBundle};
use ivy_game::{
    free_camera::{setup_camera, FreeCameraPlugin},
    ray_picker::RayPickingPlugin,
};
use ivy_graphics::texture::TextureData;
use ivy_input::layer::InputLayer;
use ivy_physics::{
    components::{collider_shape, rigid_body_type},
    ColliderBundle, PhysicsPlugin,
};
use ivy_postprocessing::preconfigured::{SurfacePbrPipelineDesc, SurfacePbrRenderer};
use ivy_wgpu::{
    components::*,
    driver::WinitDriver,
    events::ResizedEvent,
    layer::GraphicsLayer,
    light::{LightKind, LightParams},
    material_desc::{MaterialData, PbrMaterialData},
    mesh_desc::MeshDesc,
    primitives::{CapsulePrimitive, CubePrimitive, UvSpherePrimitive},
    renderer::{EnvironmentData, RenderObjectBundle},
    shaders::{PbrShaderDesc, ShadowShaderDesc},
};
use rapier3d::prelude::{RigidBodyType, SharedShape};
use tracing_subscriber::{layer::SubscriberExt, registry, util::SubscriberInitExt, EnvFilter};
use tracing_tree::HierarchicalLayer;
use winit::{dpi::LogicalSize, window::WindowAttributes};

const ENABLE_SKYBOX: bool = true;

pub fn main() -> anyhow::Result<()> {
    color_backtrace::install();

    registry()
        .with(EnvFilter::from_default_env())
        .with(
            HierarchicalLayer::default()
                .with_indent_lines(true)
                .with_span_retrace(true),
        )
        .init();

    if let Err(err) = App::builder()
        .with_driver(WinitDriver::new(
            WindowAttributes::default()
                .with_inner_size(LogicalSize::new(1920, 1080))
                .with_title("Ivy Physics"),
        ))
        .with_layer(EngineLayer::new())
        .with_layer(ProfilingLayer::new())
        .with_layer(GraphicsLayer::new(|world, assets, gpu, surface| {
            Ok(SurfacePbrRenderer::new(
                world,
                assets,
                gpu,
                surface,
                SurfacePbrPipelineDesc {
                    hdri: Some(Box::new(AssetPath::new(
                        "hdris/kloofendal_48d_partly_cloudy_puresky_2k.hdr",
                    ))),
                    ..Default::default()
                },
            ))
        }))
        .with_layer(InputLayer::new())
        .with_layer(LogicLayer)
        .with_layer(ScheduledLayer::new(FixedTimeStep::new(0.02)).with_plugin(FreeCameraPlugin))
        .with_layer(
            ScheduledLayer::new(FixedTimeStep::new(0.02))
                .with_plugin(PhysicsPlugin::new().with_gravity(-Vec3::Y * 9.81))
                .with_plugin(RayPickingPlugin),
        )
        .run()
    {
        tracing::error!("{err:?}");
        Err(err)
    } else {
        Ok(())
    }
}

fn setup_objects(world: &mut World, assets: AssetCache) -> anyhow::Result<()> {
    let white_material = MaterialData::PbrMaterial(
        PbrMaterialData::new()
            .with_roughness_factor(1.0)
            .with_metallic_factor(0.0)
            .with_albedo(TextureData::srgba(Srgba::new(1.0, 1.0, 1.0, 1.0))),
    );

    let red_material = MaterialData::PbrMaterial(
        PbrMaterialData::new()
            .with_roughness_factor(1.0)
            .with_metallic_factor(0.0)
            .with_albedo(TextureData::srgba(Color::from_hsla(1.0, 0.7, 0.7, 1.0))),
    );

    const RESTITUTION: f32 = 0.1;
    const FRICTION: f32 = 0.8;

    let body = || {
        let mut builder = Entity::builder();
        builder
            .mount(TransformBundle::default())
            .mount(RigidBodyBundle::new(RigidBodyType::Dynamic))
            .mount(
                ColliderBundle::new(SharedShape::cuboid(1.0, 1.0, 1.0))
                    .with_restitution(RESTITUTION)
                    .with_friction(FRICTION),
            )
            .mount(RenderObjectBundle::new(
                MeshDesc::Content(assets.load(&CubePrimitive)),
                &[
                    (forward_pass(), red_material.clone()),
                    (shadow_pass(), MaterialData::ShadowMaterial),
                ],
            ));

        builder
    };

    let cube = |pos: Vec3, size: Vec3| {
        let mut builder = body();
        builder.set(ivy_core::components::position(), pos).set(
            collider_shape(),
            SharedShape::cuboid(size.x, size.y, size.z),
        );
        builder
    };

    let sphere = |pos: Vec3, size: f32| {
        let mut builder = body();
        builder
            .set(ivy_core::components::position(), pos)
            .set(
                mesh(),
                MeshDesc::Content(assets.load(&UvSpherePrimitive::default())),
            )
            .set(collider_shape(), SharedShape::ball(size));
        builder
    };

    let capsule = |pos: Vec3| {
        let mut builder = body();
        builder
            .set(ivy_core::components::position(), pos)
            .set(
                mesh(),
                MeshDesc::Content(assets.load(&CapsulePrimitive::default())),
            )
            .set(collider_shape(), SharedShape::capsule_y(1.0, 1.0));
        builder
    };

    cube(Vec3::ZERO, vec3(100.0, 1.0, 100.0))
        .set(rigid_body_type(), RigidBodyType::Fixed)
        .set(scale(), vec3(100.0, 1.0, 100.0))
        .set(is_static(), ())
        .set(forward_pass(), white_material)
        .spawn(world);

    let drop_height = 10.0;

    cube(vec3(0.0, drop_height, 0.0), Vec3::ONE)
        .set(rotation(), Quat::from_scaled_axis(vec3(1.0, 1.0, 0.0)))
        .spawn(world);

    cube(vec3(5.0, drop_height, 0.0), Vec3::ONE)
        .set(rotation(), Quat::from_scaled_axis(vec3(1.0, 0.0, 0.0)))
        .spawn(world);

    sphere(vec3(10.0, drop_height, 0.0), 1.0)
        .set(rotation(), Quat::from_scaled_axis(vec3(0.0, 0.0, 0.0)))
        .spawn(world);

    capsule(vec3(-5.0, drop_height, 0.0))
        .set(rotation(), Quat::from_scaled_axis(vec3(0.1, 0.0, 0.0)))
        .spawn(world);

    capsule(vec3(-10.0, drop_height, 0.0))
        .set(rotation(), Quat::from_scaled_axis(vec3(0.0, 0.0, 1.0)))
        .spawn(world);

    for i in 0..4 {
        cube(
            vec3(0.0 + i as f32 * 0.0, 2.0 + i as f32 * 2.0, -8.0),
            Vec3::ONE,
        )
        .set(rotation(), Quat::from_scaled_axis(vec3(0.0, 0.0, 0.0)))
        .spawn(world);
    }

    Entity::builder()
        .mount(TransformBundle::default().with_rotation(Quat::from_euler(
            EulerRot::YXZ,
            -2.0,
            -1.0,
            0.0,
        )))
        .set(
            light_params(),
            LightParams::new(Srgb::new(1.0, 1.0, 1.0), 1.0),
        )
        .set(light_kind(), LightKind::Directional)
        .set_default(cast_shadow())
        .spawn(world);

    Ok(())
}

struct LogicLayer;

impl Layer for LogicLayer {
    fn register(
        &mut self,
        world: &mut World,
        _: &AssetCache,
        mut events: EventRegisterContext<Self>,
    ) -> anyhow::Result<()> {
        events.subscribe(|_, world, assets, _: &PostInitEvent| {
            setup_objects(world, assets.clone())?;

            Ok(())
        });

        events.subscribe(|_, world, _, resized: &ResizedEvent| {
            if let Some(main_camera) = Query::new(projection_matrix().as_mut())
                .with(main_camera())
                .borrow(world)
                .first()
            {
                let aspect =
                    resized.physical_size.width as f32 / resized.physical_size.height as f32;
                *main_camera = Mat4::perspective_rh(1.0, aspect, 0.01, 1000.0);
            }

            Ok(())
        });

        setup_camera()
            .mount(TransformBundle::new(
                vec3(0.0, 20.0, 20.0),
                Quat::IDENTITY,
                Vec3::ONE,
            ))
            .set(
                environment_data(),
                EnvironmentData::new(
                    Srgb::new(0.2, 0.2, 0.3),
                    0.001,
                    if ENABLE_SKYBOX { 0.0 } else { 1.0 },
                ),
            )
            .spawn(world);

        Ok(())
    }
}

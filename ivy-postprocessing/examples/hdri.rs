use std::process::Output;

use ivy_assets::AssetCache;
use ivy_postprocessing::hdri::process_hdri;
use ivy_wgpu::{types::texture::read_texture, Gpu};
use tracing_subscriber::fmt::init;
use wgpu::{Extent3d, ImageCopyTexture, TextureFormat};

fn main() {
    tracing_subscriber::fmt::init();

    async_std::task::block_on(async {
        let gpu = Gpu::headless().await;

        let assets = AssetCache::new();
        assets.register_service(gpu.clone());

        tracing::info!("loading image");
        let image = image::open("./ivy-postprocessing/hdrs/lauter_waterfall_4k.hdr").unwrap();

        let mut encoder = gpu.device.create_command_encoder(&Default::default());

        tracing::info!("processing hdri");
        let hdri = process_hdri(
            &gpu,
            &mut encoder,
            &assets,
            &image,
            TextureFormat::Rgba8UnormSrgb,
        );

        gpu.queue.submit([encoder.finish()]);

        tracing::info!("finished");
        let output = read_texture(&gpu, &hdri, 0, 0, image::ColorType::Rgba8).await;
        output.unwrap().save("output.png").unwrap();
    });
}

use std::sync::Arc;

use anyhow::Context;
use ash::vk::DescriptorSet;
use flax::{Component, World};
use ivy_assets::AssetCache;
use ivy_vulkan::{commands::CommandBuffer, PassInfo, Shader};
use parking_lot::Mutex;

// Generic interface provided for the base renderer
// TODO: remove this trait/simplity and use nodes instead
pub trait Renderer {
    // Draws the scene using the pass [`Pass`] and the provided camera.
    // NOTE: camera must have gpu side data.
    fn draw(
        &mut self,
        world: &mut World,
        assets: &AssetCache,
        cmd: &CommandBuffer,
        // Descriptor sets to bind before renderer specific sets
        sets: &[DescriptorSet],
        // Information about the current pass
        pass_info: &PassInfo,
        // Dynamic offsets for supplied sets
        offsets: &[u32],
        // The current swapchain image or backbuffer index
        current_frame: usize,
        // TODO: abstract specific rendering into a node and only use renderers as a broader
        // concept
        pass: Component<Shader>,
    ) -> anyhow::Result<()>;
}

impl<T> Renderer for Arc<Mutex<T>>
where
    T: Renderer,
{
    fn draw(
        &mut self,
        world: &mut World,
        assets: &AssetCache,
        cmd: &CommandBuffer,
        sets: &[DescriptorSet],
        pass_info: &PassInfo,
        offsets: &[u32],
        current_frame: usize,
        pass: Component<Shader>,
    ) -> anyhow::Result<()> {
        self.lock()
            .draw(
                world,
                assets,
                cmd,
                sets,
                pass_info,
                offsets,
                current_frame,
                pass,
            )
            .with_context(|| {
                format!(
                    "Failed to draw using renderer {:?}",
                    std::any::type_name::<T>()
                )
            })
    }
}

macro_rules! tuple_impl {
    ($($name: ident),*) => {
        impl<$($name: Renderer),*> Renderer for ($($name,)*) {
            // Draws the scene using the pass [`Pass`] and the provided camera.
            // Note: camera must have gpu side data.
            fn draw(
                &mut self,
                world: &mut World,
                assets: &AssetCache,
                cmd: &CommandBuffer,
                sets: &[DescriptorSet],
                pass_info: &PassInfo,
                offsets: &[u32],
                current_frame: usize,
                pass: Component<Shader>,
            ) -> anyhow::Result<()> {
                #[allow(non_snake_case)]
                let ($($name,)+) = self;
                ($($name
                    .draw(world, assets, cmd, sets, pass_info, offsets, current_frame, pass)
                    .with_context(|| {
                        format!(
                            "Failed to draw using renderer {:?}",
                            std::any::type_name::<$name>()
                        )
                    })
                ?),*);

                Ok(())
            }
        }
    }
}

// Implement renderer on tuple of renderers and tuple of render handles
tuple_impl! { A }
tuple_impl! { A, B }
tuple_impl! { A, B, C }
tuple_impl! { A, B, C, D }
tuple_impl! { A, B, C, D, E }

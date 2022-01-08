#![allow(non_snake_case)]
use std::marker::PhantomData;

use anyhow::Context;
use ash::vk::DescriptorSet;
use hecs::World;
use ivy_resources::{Handle, Resources, Storage};
use ivy_vulkan::{commands::CommandBuffer, shaderpass::ShaderPass, PassInfo};

// Generic interface for a renderer.
pub trait Renderer {
    type Error;
    // Draws the scene using the pass [`Pass`] and the provided camera.
    // Note: camera must have gpu side data.
    fn draw<Pass: ShaderPass>(
        &mut self,
        // The ecs world
        world: &mut World,
        // Graphics resources like textures and materials
        resources: &Resources,
        // The commandbuffer to record into
        cmd: &CommandBuffer,
        // Descriptor sets to bind before renderer specific sets
        sets: &[DescriptorSet],
        // Information about the current pass
        pass_info: &PassInfo,
        // Dynamic offsets for supplied sets
        offsets: &[u32],
        // The current swapchain image or backbuffer index
        current_frame: usize,
    ) -> Result<(), Self::Error>;
}

/// Override the pass which is used to draw the renderer
pub struct WithPass<Pass, R> {
    renderer: R,
    pass: PhantomData<Pass>,
}

impl<Pass, R> WithPass<Pass, R>
where
    Pass: ShaderPass,
    R: Renderer,
{
    pub fn new(renderer: R) -> Self {
        Self {
            renderer,
            pass: PhantomData,
        }
    }
}

impl<Pass: ShaderPass, E, R> Renderer for WithPass<Pass, R>
where
    R: Renderer<Error = E>,
{
    type Error = E;

    fn draw<Ignored: ShaderPass>(
        &mut self,
        world: &mut World,
        resources: &Resources,
        cmd: &CommandBuffer,
        sets: &[DescriptorSet],
        pass_info: &PassInfo,
        offsets: &[u32],
        current_frame: usize,
    ) -> Result<(), Self::Error> {
        self.renderer.draw::<Pass>(
            world,
            resources,
            cmd,
            sets,
            pass_info,
            offsets,
            current_frame,
        )
    }
}

impl<E, T> Renderer for Handle<T>
where
    E: Into<anyhow::Error>,
    T: Renderer<Error = E> + Storage,
{
    type Error = anyhow::Error;

    fn draw<Pass: ShaderPass>(
        &mut self,
        world: &mut World,
        resources: &Resources,
        cmd: &CommandBuffer,
        sets: &[DescriptorSet],
        pass_info: &PassInfo,
        offsets: &[u32],
        current_frame: usize,
    ) -> Result<(), Self::Error> {
        resources
            .get_mut(*self)
            .with_context(|| {
                format!(
                    "Failed to get renderer {:?} from handle",
                    std::any::type_name::<T>()
                )
            })?
            .draw::<Pass>(
                world,
                resources,
                cmd,
                sets,
                pass_info,
                offsets,
                current_frame,
            )
            .map_err(|e| e.into())
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
        impl<Err: Into<anyhow::Error>, $($name: Renderer<Error = Err> + ivy_resources::Storage),*> Renderer for ($($name,)*) {
            type Error = anyhow::Error;
            // Draws the scene using the pass [`Pass`] and the provided camera.
            // Note: camera must have gpu side data.
            fn draw<Pass: ShaderPass>(
                &mut self,
                world: &mut World,
                resources: &Resources,
                cmd: &CommandBuffer,
                sets: &[DescriptorSet],
                pass_info: &PassInfo,
                offsets: &[u32],
                current_frame: usize,
            ) -> Result<(), Self::Error> {
                #[allow(non_snake_case)]
                let ($($name,)+) = self;
                ($($name
                    .draw::<Pass>(world, resources, cmd, sets, pass_info, offsets, current_frame)
                    .map_err(|e| e.into())
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

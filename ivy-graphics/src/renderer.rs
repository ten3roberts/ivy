use crate::ShaderPass;
use anyhow::Context;
use ash::vk::DescriptorSet;
use hecs::World;
use ivy_resources::{Handle, Resources, Storage};
use ivy_vulkan::commands::CommandBuffer;

// Generic interface for a renderer.
pub trait Renderer {
    type Error;
    // Draws the scene using the pass [`Pass`] and the provided camera.
    // Note: camera must have gpu side data.
    fn draw<Pass: ShaderPass>(
        &mut self,
        // The ecs world
        world: &mut World,
        // The commandbuffer to record into
        cmd: &CommandBuffer,
        // The current swapchain image or backbuffer index
        current_frame: usize,
        // Descriptor sets to bind before renderer specific sets
        sets: &[DescriptorSet],
        // Dynamic offsets for supplied sets
        offsets: &[u32],
        // Graphics resources like textures and materials
        resources: &Resources,
    ) -> Result<(), Self::Error>;
}

impl<E, T> Renderer for Handle<T>
where
    E: Into<anyhow::Error>,
    T: Renderer<Error = E> + Storage,
{
    type Error = anyhow::Error;

    fn draw<Pass: ShaderPass>(
        &mut self,
        // The ecs world
        world: &mut World,
        // The commandbuffer to record into
        cmd: &CommandBuffer,
        // The current swapchain image or backbuffer index
        current_frame: usize,
        // Descriptor sets to bind before renderer specific sets
        sets: &[DescriptorSet],
        // Dynamic offsets for supplied sets
        offsets: &[u32],
        // Graphics resources like textures and materials
        resources: &Resources,
    ) -> Result<(), Self::Error> {
        resources
            .get_mut(*self)
            .with_context(|| {
                format!(
                    "Failed to get renderer {:?} from handle",
                    std::any::type_name::<T>()
                )
            })?
            .draw::<Pass>(world, cmd, current_frame, sets, offsets, resources)
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
    // impl<Err: 'static + Send + Sync + std::error::Error, $($name: Renderer<Error = Err>),*> Renderer for ($($name,)*) {
    //     type Error = anyhow::Error;
    //     // Draws the scene using the pass [`Pass`] and the provided camera.
    //     // Note: camera must have gpu side data.
    //     fn draw<Pass: ShaderPass>(
    //         &mut self,
    //         // The ecs world
    //         world: &mut World,
    //         // The commandbuffer to record into
    //         cmd: &CommandBuffer,
    //         // The current swapchain image or backbuffer index
    //         current_frame: usize,
    //         // Descriptor sets to bind before renderer specific sets
    //         sets: &[DescriptorSet],
    //         // Dynamic offsets for supplied sets
    //         offsets: &[u32],
    //         // Graphics resources like textures and materials
    //         resources: &Resources,
    //     ) -> Result<(), Self::Error> {
    //             #[allow(non_snake_case)]
    //             let ($($name,)+) = self;
    //             ($($name.draw::<Pass>(world, cmd, current_frame, sets, offsets, resources)?),*);
    //             Ok(())
    //     }
    // }

    impl<Err: 'static + Send + Sync + std::error::Error, $($name: Renderer<Error = Err> + ivy_resources::Storage),*> Renderer for ($(ivy_resources::Handle<$name>,)*) {
        type Error = anyhow::Error;
        // Draws the scene using the pass [`Pass`] and the provided camera.
        // Note: camera must have gpu side data.
        fn draw<Pass: ShaderPass>(
            &mut self,
            // The ecs world
            world: &mut World,
            // The commandbuffer to record into
            cmd: &CommandBuffer,
            // The current swapchain image or backbuffer index
            current_frame: usize,
            // Descriptor sets to bind before renderer specific sets
            sets: &[DescriptorSet],
            // Dynamic offsets for supplied sets
            offsets: &[u32],
            // Graphics resources like textures and materials
            resources: &Resources,
        ) -> Result<(), Self::Error> {
    #[allow(non_snake_case)]
                let ($($name,)+) = self;
                ($((resources.get_mut(*($name))
                    .with_context(|| {
                        format!(
                            "Failed to get renderer {:?} from handle",
                            std::any::type_name::<$name>()
                        )
                    })?
                    .draw::<Pass>(world, cmd, current_frame, sets, offsets, resources)
                    .with_context(|| {
                        format!(
                            "Failed to draw using renderer {:?}",
                            std::any::type_name::<$name>()
                        )
                    })

                )?),*);
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
tuple_impl! { A, B, C, D, E, F }
tuple_impl! { A, B, C, D, E, F, G }
tuple_impl! { A, B, C, D, E, F, G, H }
tuple_impl! { A, B, C, D, E, F, G, H, I }
tuple_impl! { A, B, C, D, E, F, G, H, I, J }
tuple_impl! { A, B, C, D, E, F, G, H, I, J, K }
tuple_impl! { A, B, C, D, E, F, G, H, I, J, K, L }

use anyhow::Context;
use ash::vk::DescriptorSet;
use flax::{Component, World};
use ivy_resources::{Handle, Resources, Storage};
use ivy_vulkan::{commands::CommandBuffer, PassInfo, Shader};

// Generic interface provided for the base renderer
// TODO: remove this trait/simplity and use nodes instead
pub trait Renderer {
    // Draws the scene using the pass [`Pass`] and the provided camera.
    // NOTE: camera must have gpu side data.
    fn draw(
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
        // TODO: abstract specific rendering into a node and only use renderers as a broader
        // concept
        pass: Component<Shader>,
    ) -> anyhow::Result<()>;
}

impl<T> Renderer for Handle<T>
where
    T: Renderer + Storage,
{
    fn draw(
        &mut self,
        world: &mut World,
        resources: &Resources,
        cmd: &CommandBuffer,
        sets: &[DescriptorSet],
        pass_info: &PassInfo,
        offsets: &[u32],
        current_frame: usize,
        pass: Component<Shader>,
    ) -> anyhow::Result<()> {
        resources
            .get_mut(*self)
            .with_context(|| {
                format!(
                    "Failed to get renderer {:?} from handle",
                    std::any::type_name::<T>()
                )
            })?
            .draw(
                world,
                resources,
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
        impl<$($name: Renderer + ivy_resources::Storage),*> Renderer for ($($name,)*) {
            // Draws the scene using the pass [`Pass`] and the provided camera.
            // Note: camera must have gpu side data.
            fn draw(
                &mut self,
                world: &mut World,
                resources: &Resources,
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
                    .draw(world, resources, cmd, sets, pass_info, offsets, current_frame, pass)
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

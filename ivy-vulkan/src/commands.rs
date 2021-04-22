use std::rc::Rc;

use crate::ClearValue;

use super::pipeline::Pipeline;
use super::renderpass::RenderPass;
use super::Error;
use super::{
    buffer::{Buffer, BufferType},
    device,
};
use super::{framebuffer::Framebuffer, Extent};
use arrayvec::ArrayVec;
use ash::vk;
use ash::Device;
use ash::{version::DeviceV1_0, vk::PipelineLayout};

/// Maximum number of bound vertex buffers
/// This is required to avoid dynamically allocating a list of buffers when
/// binding
pub const MAX_VB_BINDING: usize = 4;

pub struct CommandPool {
    device: Rc<Device>,
    commandpool: vk::CommandPool,
}

/// `transient`: Commandbuffers allocated are very shortlived
/// `reset`: Commandbuffers can be individually reset from pool
impl CommandPool {
    pub fn new(
        device: Rc<Device>,
        queue_family: u32,
        transient: bool,
        reset: bool,
    ) -> Result<Self, Error> {
        let flags = if transient {
            vk::CommandPoolCreateFlags::TRANSIENT
        } else {
            vk::CommandPoolCreateFlags::default()
        } | if reset {
            vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER
        } else {
            vk::CommandPoolCreateFlags::default()
        };

        let create_info = vk::CommandPoolCreateInfo::builder()
            .queue_family_index(queue_family)
            .flags(flags);

        let commandpool = unsafe { device.create_command_pool(&create_info, None)? };

        Ok(CommandPool {
            device,
            commandpool,
        })
    }

    pub fn allocate(&self, count: u32) -> Result<Vec<CommandBuffer>, Error> {
        let alloc_info = vk::CommandBufferAllocateInfo::builder()
            .command_pool(self.commandpool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(count);

        // Allocate handles
        let raw = unsafe { self.device.allocate_command_buffers(&alloc_info)? };

        // Wrap handles
        let commandbuffers = raw
            .iter()
            .map(|commandbuffer| CommandBuffer {
                device: self.device.clone(),
                commandbuffer: *commandbuffer,
            })
            .collect::<Vec<_>>();

        Ok(commandbuffers)
    }

    // Resets all command buffers allocated from pool
    // `release`: Release all memory allocated back to the system, if
    // commandbuffers are to be rerecorded, this will need to once again
    // acquire memory
    pub fn reset(&self, release: bool) -> Result<(), Error> {
        let flags = if release {
            vk::CommandPoolResetFlags::RELEASE_RESOURCES
        } else {
            vk::CommandPoolResetFlags::default()
        };

        unsafe { self.device.reset_command_pool(self.commandpool, flags)? }
        Ok(())
    }

    // Frees a single commandbuffer
    // It is more efficient to reset the whole pool rather than freeing all
    // individually
    pub fn free(&self, commandbuffer: CommandBuffer) {
        unsafe {
            self.device
                .free_command_buffers(self.commandpool, &[commandbuffer.commandbuffer])
        }
    }

    pub fn device(&self) -> &ash::Device {
        &self.device
    }

    /// Provides a context withing a single time submit commandbuffer will be recorded
    /// At the end of the function the commandbuffer is ended and submitted to the queue
    /// Will wait for queue to idle
    pub fn single_time_command<F: FnOnce(&CommandBuffer) -> R, R>(
        &self,
        queue: vk::Queue,
        func: F,
    ) -> Result<R, Error> {
        let commandbuffer = self.allocate(1)?.pop().unwrap();

        commandbuffer.begin(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT)?;

        let result = func(&commandbuffer);

        commandbuffer.end()?;
        commandbuffer.submit(queue, &[], &[], vk::Fence::null(), &[])?;

        device::queue_wait_idle(&self.device, queue)?;
        self.free(commandbuffer);

        Ok(result)
    }
}

impl Drop for CommandPool {
    fn drop(&mut self) {
        unsafe { self.device.destroy_command_pool(self.commandpool, None) }
    }
}

pub struct CommandBuffer {
    device: Rc<Device>,
    commandbuffer: vk::CommandBuffer,
}

impl CommandBuffer {
    /// Starts recording of a commandbuffer
    pub fn begin(&self, flags: vk::CommandBufferUsageFlags) -> Result<(), Error> {
        let begin_info = vk::CommandBufferBeginInfo {
            flags,
            ..Default::default()
        };

        unsafe {
            self.device
                .begin_command_buffer(self.commandbuffer, &begin_info)?
        };

        Ok(())
    }

    // Ends recording of commandbuffer
    pub fn end(&self) -> Result<(), Error> {
        unsafe { self.device.end_command_buffer(self.commandbuffer)? };
        Ok(())
    }

    // Begins a renderpass
    pub fn begin_renderpass(
        &self,
        renderpass: &RenderPass,
        framebuffer: &Framebuffer,
        extent: Extent,
        clear_values: &[ClearValue],
    ) {
        let clear_values = clear_values
            .iter()
            .map(|val| val.into())
            .collect::<ArrayVec<[_; 8]>>();

        let begin_info = vk::RenderPassBeginInfo {
            s_type: vk::StructureType::RENDER_PASS_BEGIN_INFO,
            p_next: std::ptr::null(),
            render_pass: renderpass.renderpass(),
            framebuffer: framebuffer.framebuffer(),
            render_area: vk::Rect2D {
                offset: vk::Offset2D { x: 0, y: 0 },
                extent: extent.into(),
            },
            clear_value_count: clear_values.len() as _,
            p_clear_values: clear_values.as_ptr(),
        };

        unsafe {
            self.device.cmd_begin_render_pass(
                self.commandbuffer,
                &begin_info,
                vk::SubpassContents::INLINE,
            )
        }
    }

    // Ends current renderpass
    pub fn end_renderpass(&self) {
        unsafe { self.device.cmd_end_render_pass(self.commandbuffer) }
    }

    // Binds a graphics pipeline
    pub fn bind_pipeline(&self, pipeline: &Pipeline) {
        unsafe {
            self.device.cmd_bind_pipeline(
                self.commandbuffer,
                vk::PipelineBindPoint::GRAPHICS,
                pipeline.pipeline(),
            )
        }
    }

    pub fn bind_vertexbuffers<B: AsRef<vk::Buffer>>(
        &self,
        first_binding: u32,
        vertexbuffers: &[B],
    ) {
        let buffers: ArrayVec<[vk::Buffer; MAX_VB_BINDING]> =
            vertexbuffers.iter().map(|vb| *vb.as_ref()).collect();

        unsafe {
            self.device.cmd_bind_vertex_buffers(
                self.commandbuffer,
                first_binding,
                &buffers,
                &[0; MAX_VB_BINDING][0..buffers.len()],
            )
        }
    }

    pub fn bind_indexbuffer(&self, indexbuffer: &Buffer, offset: vk::DeviceSize) {
        let index_type = match indexbuffer.ty() {
            BufferType::Index16 => vk::IndexType::UINT16,
            BufferType::Index32 => vk::IndexType::UINT32,
            _ => vk::IndexType::NONE_KHR,
        };

        unsafe {
            self.device.cmd_bind_index_buffer(
                self.commandbuffer,
                indexbuffer.buffer(),
                offset,
                index_type,
            )
        }
    }

    pub fn bind_descriptor_sets(
        &self,
        pipeline_layout: PipelineLayout,
        first_set: u32,
        descriptor_sets: &[vk::DescriptorSet],
    ) {
        unsafe {
            self.device.cmd_bind_descriptor_sets(
                self.commandbuffer,
                vk::PipelineBindPoint::GRAPHICS,
                pipeline_layout,
                first_set,
                descriptor_sets,
                &[],
            )
        }
    }

    // Issues a draw command using the currently vertex buffer
    pub fn draw(
        &self,
        vertex_count: u32,
        instance_count: u32,
        first_vertex: u32,
        instance_offset: u32,
    ) {
        unsafe {
            self.device.cmd_draw(
                self.commandbuffer,
                vertex_count,
                instance_count,
                first_vertex,
                instance_offset,
            )
        }
    }

    // Issues a draw command using the currently bound vertex and index buffers
    pub fn draw_indexed(
        &self,
        index_count: u32,
        instance_count: u32,
        first_index: u32,
        vertex_offset: i32,
        first_instance: u32,
    ) {
        unsafe {
            self.device.cmd_draw_indexed(
                self.commandbuffer,
                index_count,
                instance_count,
                first_index,
                vertex_offset,
                first_instance,
            )
        }
    }

    pub fn copy_buffer(&self, src: vk::Buffer, dst: vk::Buffer, regions: &[vk::BufferCopy]) {
        unsafe {
            self.device
                .cmd_copy_buffer(self.commandbuffer, src, dst, regions)
        }
    }

    /// Copies a buffer to an image
    pub fn copy_buffer_image(
        &self,
        src: vk::Buffer,
        dst: vk::Image,
        layout: vk::ImageLayout,
        regions: &[vk::BufferImageCopy],
    ) {
        unsafe {
            self.device
                .cmd_copy_buffer_to_image(self.commandbuffer, src, dst, layout, regions)
        }
    }

    pub fn pipeline_barrier(
        &self,
        src_stage_mask: vk::PipelineStageFlags,
        dst_stage_mask: vk::PipelineStageFlags,
        image_barriers: &[vk::ImageMemoryBarrier],
    ) {
        unsafe {
            self.device.cmd_pipeline_barrier(
                self.commandbuffer,
                src_stage_mask,
                dst_stage_mask,
                vk::DependencyFlags::default(),
                &[],
                &[],
                image_barriers,
            )
        }
    }

    pub fn blit_image(
        &self,
        src: vk::Image,
        src_layout: vk::ImageLayout,
        dst: vk::Image,
        dst_layout: vk::ImageLayout,
        regions: &[vk::ImageBlit],
        filter: vk::Filter,
    ) {
        unsafe {
            self.device.cmd_blit_image(
                self.commandbuffer,
                src,
                src_layout,
                dst,
                dst_layout,
                regions,
                filter,
            )
        }
    }

    pub fn submit_multiple(
        device: &Device,
        commandbuffers: &[vk::CommandBuffer],
        queue: vk::Queue,
        wait_semaphores: &[vk::Semaphore],
        signal_semaphores: &[vk::Semaphore],
        fence: vk::Fence,
        wait_stages: &[vk::PipelineStageFlags],
    ) -> Result<(), Error> {
        let submit_info = vk::SubmitInfo {
            s_type: vk::StructureType::SUBMIT_INFO,
            p_next: std::ptr::null(),
            wait_semaphore_count: wait_semaphores.len() as u32,
            p_wait_semaphores: wait_semaphores.as_ptr(),
            p_wait_dst_stage_mask: wait_stages.as_ptr(),
            command_buffer_count: commandbuffers.len() as u32,
            p_command_buffers: commandbuffers.as_ptr(),
            signal_semaphore_count: signal_semaphores.len() as u32,
            p_signal_semaphores: signal_semaphores.as_ptr(),
        };

        unsafe { device.queue_submit(queue, &[submit_info], fence) }?;

        Ok(())
    }

    /// Submits a single commandbuffer.
    pub fn submit(
        &self,
        queue: vk::Queue,
        wait_semaphores: &[vk::Semaphore],
        signal_semaphores: &[vk::Semaphore],
        fence: vk::Fence,
        wait_stages: &[vk::PipelineStageFlags],
    ) -> Result<(), Error> {
        let submit_info = vk::SubmitInfo {
            s_type: vk::StructureType::SUBMIT_INFO,
            p_next: std::ptr::null(),
            wait_semaphore_count: wait_semaphores.len() as _,
            p_wait_semaphores: wait_semaphores.as_ptr(),
            p_wait_dst_stage_mask: wait_stages.as_ptr(),
            command_buffer_count: 1,
            p_command_buffers: &self.commandbuffer,
            signal_semaphore_count: signal_semaphores.len() as _,
            p_signal_semaphores: signal_semaphores.as_ptr(),
        };

        unsafe { self.device.queue_submit(queue, &[submit_info], fence) }?;

        Ok(())
    }
}

impl AsRef<vk::CommandBuffer> for CommandBuffer {
    fn as_ref(&self) -> &vk::CommandBuffer {
        &self.commandbuffer
    }
}

impl Into<vk::CommandBuffer> for &CommandBuffer {
    fn into(self) -> vk::CommandBuffer {
        self.commandbuffer
    }
}

use std::{marker::PhantomData, ops::Deref};

use ivy_graphics::Result;
use ivy_rendergraph::{AttachmentInfo, Node, NodeKind};
use ivy_resources::{Handle, Resources};
use ivy_vulkan::{context::SharedVulkanContext, descriptors::*, vk::ShaderStageFlags, *};
use once_cell::sync::OnceCell;

pub struct PostProcessingNode<Pass> {
    sets: Option<Vec<DescriptorSet>>,
    read_attachments: Vec<Handle<Texture>>,
    input_attachments: Vec<Handle<Texture>>,
    color_attachments: Vec<AttachmentInfo>,
    sampler: Sampler,
    marker: PhantomData<Pass>,
    pipeline: OnceCell<Pipeline>,
}

/// Creates a post processing node that will execute using the default shaderpass of the provided
/// type.
/// A descriptor for each frame in flight referencing all `read_attachments`, `input_attachments`, and
/// `bindables` in order are automatically created, and need to be matched in the shader at set
/// = 0;
impl<Pass: 'static + ShaderPass> PostProcessingNode<Pass> {
    pub fn new(
        context: SharedVulkanContext,
        resources: &Resources,
        read_attachments: &[Handle<Texture>],
        input_attachments: &[Handle<Texture>],
        bindables: &[&dyn MultiDescriptorBindable],
        color_attachments: &[AttachmentInfo],
        frames_in_flight: usize,
    ) -> Result<Self> {
        let sampler = Sampler::new(
            context.clone(),
            &SamplerInfo {
                address_mode: AddressMode::CLAMP_TO_EDGE,
                mag_filter: FilterMode::LINEAR,
                min_filter: FilterMode::LINEAR,
                unnormalized_coordinates: false,
                anisotropy: 16,
                mip_levels: 1,
            },
        )?;

        let combined_image_samplers = read_attachments
            .iter()
            .map(|val| -> Result<_> {
                Ok(CombinedImageSampler::new(
                    resources.get(*val)?.deref(),
                    &sampler,
                ))
            })
            .collect::<Result<Vec<_>>>()?;

        let input_bindabled = input_attachments
            .iter()
            .map(|val| -> Result<_> { Ok(InputAttachment::new(resources.get(*val)?.deref())) })
            .collect::<Result<Vec<_>>>()?;

        let bindables = combined_image_samplers
            .iter()
            .map(|val| val as &dyn MultiDescriptorBindable)
            .chain(
                input_bindabled
                    .iter()
                    .map(|val| val as &dyn MultiDescriptorBindable),
            )
            .chain(bindables.into_iter().cloned())
            .map(|val| (val, ShaderStageFlags::FRAGMENT))
            .collect::<Vec<_>>();

        let sets = if !bindables.is_empty() {
            Some(DescriptorBuilder::from_mutliple_resources(
                &context,
                &bindables,
                frames_in_flight,
            )?)
        } else {
            None
        };

        Ok(Self {
            pipeline: OnceCell::new(),
            sets,
            read_attachments: read_attachments.to_owned(),
            input_attachments: input_attachments.to_owned(),
            color_attachments: color_attachments.to_owned(),
            sampler,
            marker: PhantomData,
        })
    }

    /// Get a reference to the post processing node's sampler.
    pub fn sampler(&self) -> &Sampler {
        &self.sampler
    }
}

impl<Pass: ShaderPass> Node for PostProcessingNode<Pass> {
    fn color_attachments(&self) -> &[AttachmentInfo] {
        &self.color_attachments
    }

    fn read_attachments(&self) -> &[Handle<Texture>] {
        &self.read_attachments
    }

    fn input_attachments(&self) -> &[Handle<Texture>] {
        &self.input_attachments
    }

    fn depth_attachment(&self) -> Option<&AttachmentInfo> {
        None
    }

    fn clear_values(&self) -> &[ivy_vulkan::vk::ClearValue] {
        &[]
    }

    fn node_kind(&self) -> NodeKind {
        NodeKind::Graphics
    }

    fn execute(
        &mut self,
        _: &mut hecs::World,
        resources: &Resources,
        cmd: &ivy_vulkan::commands::CommandBuffer,
        pass_info: &PassInfo,
        current_frame: usize,
    ) -> anyhow::Result<()> {
        let pipeline = self.pipeline.get_or_try_init(|| {
            let context = resources.get_default::<SharedVulkanContext>()?;
            let pass = resources.get_default::<Pass>()?;
            Pipeline::new::<()>(context.clone(), pass.pipeline(), pass_info)
        })?;

        cmd.bind_pipeline(pipeline);

        if let Some(sets) = &self.sets {
            cmd.bind_descriptor_sets(pipeline.layout(), 0, &[sets[current_frame]], &[]);
        }

        cmd.draw(3, 1, 0, 0);

        Ok(())
    }

    fn debug_name(&self) -> &'static str {
        "Post Processing"
    }
}

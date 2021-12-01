use ash::vk::PipelineLayout;

/// Represents a single pass containing the pipeline and other data. Since
/// [Material](ivy-graphics::Material) does not
/// contain a pipeline, a `ShaderPass` can be considered a master material.
pub trait ShaderPass: 'static + Send + Sync {
    /// Returns the pipeline used for this shaderpass.
    fn pipeline(&self) -> &crate::Pipeline;

    /// Returns the pipeline layout used for this shaderpass.
    fn pipeline_layout(&self) -> PipelineLayout;
}

/// Macro to create a strongly typed shaderpass.
#[macro_export(local_inner_macros)]
macro_rules! new_shaderpass {
    ( $(#[$outer:meta])* $vis:vis struct $name:ident; $($rest:tt)* ) => {
        $(#[$outer])*
        #[repr(transparent)]
        $vis struct $name(pub $crate::Pipeline);

        impl $name {
            fn new(pipeline: $crate::Pipeline) -> Self {
                Self ( pipeline )
            }
        }

        impl $crate::ShaderPass for $name {
            fn pipeline(&self) -> &$crate::Pipeline {
                &self.0
            }

            fn pipeline_layout(&self) -> $crate::vk::PipelineLayout {
                self.0.layout()
            }
        }

        $crate::new_shaderpass!($($rest)*);
    };

    ( $(#[$outer:meta])* $vis:vis struct $name:ident($($fields:tt)*); $($rest:tt)* ) => {
        $(#[$outer])*
        $vis struct $name(pub $crate::Pipeline, $($fields)* );

        impl $crate::ShaderPass for $name {
            fn pipeline(&self) -> &$crate::Pipeline {
                &self.0
            }

            fn pipeline_layout(&self) -> $crate::vk::PipelineLayout {
                self.0.layout()
            }
        }

        $crate::new_shaderpass!($($rest)*);
    };

    ( $(#[$outer:meta])* $vis:vis struct $name:ident{ $($fiels:tt)* }; $($rest:tt)* ) => {
        $(#[$outer])*
        $vis struct $name{ pub pipeline: $crate::Pipeline $($fields)* };

        impl $crate::ShaderPass for $name {
            fn pipeline(&self) -> &$crate::Pipeline {
                &self.pipeline
            }

            fn pipeline_layout(&self) -> $crate::vk::PipelineLayout {
                self.pipeline.layout()
            }
        }

        $crate::new_shaderpass!($($rest)*);
    };
    () => {}
}

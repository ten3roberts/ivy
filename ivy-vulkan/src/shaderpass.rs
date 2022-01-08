/// Represents a single pass containing the pipeline and other data. Since
/// [Material](ivy-graphics::Material) does not
/// contain a pipeline, a `ShaderPass` can be considered a master material.
pub trait ShaderPass: 'static + Send + Sync {
    /// Returns the pipeline used for this shaderpass.
    fn pipeline(&self) -> &crate::PipelineInfo;
}

/// Macro to create a strongly typed shaderpass.
#[macro_export(local_inner_macros)]
macro_rules! new_shaderpass {
    ( $(#[$outer:meta])* $vis:vis struct $name:ident; $($rest:tt)* ) => {
        $(#[$outer])*
        #[repr(transparent)]
        $vis struct $name(pub $crate::PipelineInfo);

        impl $name {
            fn new(pipeline: $crate::PipelineInfo) -> Self {
                Self ( pipeline )
            }
        }

        impl $crate::ShaderPass for $name {
            fn pipeline(&self) -> &$crate::PipelineInfo {
                &self.0
            }
        }

        $crate::new_shaderpass!($($rest)*);
    };

    ( $(#[$outer:meta])* $vis:vis struct $name:ident($($fields:tt)*); $($rest:tt)* ) => {
        $(#[$outer])*
        $vis struct $name(pub $crate::PipelineInfo, $($fields)* );

        impl $crate::ShaderPass for $name {
            fn pipeline(&self) -> &$crate::PipelineInfo {
                &self.0
            }
        }

        $crate::new_shaderpass!($($rest)*);
    };

    ( $(#[$outer:meta])* $vis:vis struct $name:ident{ $($fiels:tt)* }; $($rest:tt)* ) => {
        $(#[$outer])*
        $vis struct $name{ pub pipeline: $crate::PipelineInfo, $($fields)* };

        impl $crate::ShaderPass for $name {
            fn pipeline(&self) -> &$crate::PipelineInfo {
                &self.pipeline
            }
        }

        $crate::new_shaderpass!($($rest)*);
    };
    () => {}
}

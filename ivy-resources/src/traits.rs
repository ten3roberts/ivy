use crate::Resources;

/// Trait for loading a resource from other resources and supplied info like
/// filename or struct.
pub trait LoadResource {
    type Info;
    type Error;

    fn load(resources: &Resources, info: &Self::Info) -> Result<Self, Self::Error>
    where
        Self: Sized;
}

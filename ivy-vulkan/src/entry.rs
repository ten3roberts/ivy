use super::error::Error;
pub use ash::version::{DeviceV1_0, EntryV1_0, InstanceV1_0};
use ash::Entry;

pub fn create() -> Result<Entry, Error> {
    unsafe { Entry::new().map_err(|_| Error::LibLoading) }
}

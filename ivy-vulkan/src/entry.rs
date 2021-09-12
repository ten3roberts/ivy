use crate::{error::Error, Result};
use ash::Entry;

pub fn create() -> Result<Entry> {
    unsafe { Entry::new().map_err(|_| Error::LibLoading) }
}

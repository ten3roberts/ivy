use crate::{error::Error, Result};
use ash::Entry;

pub fn create() -> Result<Entry> {
    unsafe { Entry::load().map_err(|_| Error::LibLoading) }
}

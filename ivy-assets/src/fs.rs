use std::{ffi::OsStr, fmt::Debug, marker::PhantomData, path::PathBuf};

use derivative::Derivative;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BytesFromPath(pub PathBuf);

impl BytesFromPath {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self(path.into())
    }
}

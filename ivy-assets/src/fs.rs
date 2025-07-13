use std::{fmt::Debug, path::PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BytesFromPath(pub PathBuf);

impl BytesFromPath {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self(path.into())
    }
}

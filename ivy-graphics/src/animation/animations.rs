use ivy_assets::Asset;

use crate::{Animation, Error, Result};

/// Stores animations by name.
/// Attached to an entity by document
#[derive(Debug, Clone, Default)]
pub struct AnimationStore {
    // Use a vec due to (usually) small number of animations
    inner: Vec<(String, Asset<Animation>)>,
}

impl AnimationStore {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn insert(&mut self, name: impl Into<String>, animation: Asset<Animation>) {
        self.inner.push((name.into(), animation));
    }

    /// Find a named animation
    pub fn find(&self, name: &str) -> Result<Asset<Animation>> {
        self.iter()
            .find(|val| val.0 == name)
            .map(|val| val.1.clone())
            .ok_or_else(|| Error::MissingAnimation(name.to_string()))
    }

    pub fn iter(&self) -> std::slice::Iter<(String, Asset<Animation>)> {
        self.inner.iter()
    }

    pub fn get(&self, index: usize) -> Result<Asset<Animation>> {
        self.inner
            .get(index)
            .map(|val| val.1.clone())
            .ok_or_else(|| Error::InvalidAnimation(index))
    }
}

impl<T> From<T> for AnimationStore
where
    T: IntoIterator<Item = (String, Asset<Animation>)>,
{
    fn from(val: T) -> Self {
        Self {
            inner: val.into_iter().collect(),
        }
    }
}

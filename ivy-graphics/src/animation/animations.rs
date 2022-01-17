use ivy_resources::Handle;

use crate::{Animation, Error, Result};

/// Stores animations by name.
/// Attached to an entity by document
#[derive(Debug, Clone, Default)]
pub struct AnimationStore {
    // Use a vec due to (usually) small number of animations
    inner: Vec<(String, Handle<Animation>)>,
}

impl AnimationStore {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn insert(&mut self, name: impl Into<String>, animation: Handle<Animation>) {
        self.inner.push((name.into(), animation));
    }

    /// Find a named animation
    pub fn find(&self, name: &str) -> Result<Handle<Animation>> {
        self.iter()
            .find(|val| val.0 == name)
            .map(|val| val.1)
            .ok_or_else(|| Error::MissingAnimation(name.to_string()))
    }

    pub fn iter(&self) -> std::slice::Iter<(String, Handle<Animation>)> {
        self.inner.iter()
    }

    pub fn get(&self, index: usize) -> Result<Handle<Animation>> {
        self.inner
            .get(index)
            .map(|val| val.1)
            .ok_or_else(|| Error::InvalidAnimation(index))
    }
}

impl<T> From<T> for AnimationStore
where
    T: IntoIterator<Item = (String, Handle<Animation>)>,
{
    fn from(val: T) -> Self {
        Self {
            inner: val.into_iter().collect(),
        }
    }
}

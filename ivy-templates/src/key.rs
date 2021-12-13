use std::borrow::Cow;

use derive_more::{From, Into};

#[cfg(feature = "serialize")]
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Eq, PartialEq, PartialOrd, Ord, Default, Hash, Into, From)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub struct TemplateKey(Cow<'static, str>);

impl TemplateKey {
    pub fn new<S: Into<Cow<'static, str>>>(name: S) -> Self {
        Self(name.into())
    }
}

impl std::ops::Deref for TemplateKey {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}

impl std::ops::DerefMut for TemplateKey {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.to_mut()
    }
}

impl From<&'static str> for TemplateKey {
    fn from(val: &'static str) -> Self {
        Self::new(val)
    }
}

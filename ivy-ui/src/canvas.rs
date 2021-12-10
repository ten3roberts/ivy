#[cfg(feature = "serialize")]
use serde::{Deserialize, Serialize};
/// A canvas works as a root marker for the UI hierarchy.
///  The actual size and projection is contained in an attached Camera.
#[derive(Default, Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub struct Canvas;

use std::collections::BTreeMap;

use futures::{stream, StreamExt};
use ivy_assets::{loadable::Loadable, AssetCache};
use ivy_editable::Editable;
use violet::core::editor;

use crate::effect_desc::{RenderEffect, RenderEffectDesc};

/// A material resource represents a collection of pass [`RenderEffect`]s
pub struct Material {
    effects: BTreeMap<String, RenderEffect>,
}

#[derive(Debug, Clone, Editable, serde::Serialize, serde::Deserialize)]
pub struct MaterialDesc {
    effects: BTreeMap<String, RenderEffectDesc>,
}

impl Loadable for MaterialDesc {
    type Output = Material;

    async fn load(&self, assets: &AssetCache) -> anyhow::Result<Self::Output> {
        let effects = self.effects.load(assets).await?;

        Ok(Material { effects })
    }
}

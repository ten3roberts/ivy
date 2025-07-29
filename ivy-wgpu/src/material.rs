use std::{collections::BTreeMap, fmt::Display};

use futures::{stream, StreamExt};
use ivy_assets::{declare_resource, loadable::Loadable, Asset, AssetCache, AssetPath, Resource};
use ivy_core::{components::color, Bundle, Color, ColorExt};
use ivy_editable::Editable;

use crate::{
    components::{forward_pass, shadow_pass, transparent_pass},
    effect_desc::{RenderEffect, RenderEffectDesc},
};

// TODO: make as generic as builtin component split
#[derive(
    Debug, Clone, Editable, serde::Serialize, serde::Deserialize, PartialEq, Eq, PartialOrd, Ord,
)]
pub enum EffectPass {
    Forward,
    Transparent,
    Shadow,
}

impl Display for EffectPass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EffectPass::Forward => write!(f, "forward"),
            EffectPass::Transparent => write!(f, "transparent"),
            EffectPass::Shadow => write!(f, "shadow"),
        }
    }
}

/// A material resource represents a collection of pass [`RenderEffect`]s
pub struct Material {
    effects: BTreeMap<EffectPass, RenderEffect>,
}

#[derive(Debug, Clone, Editable, serde::Serialize, serde::Deserialize)]
pub struct MaterialDesc {
    effects: BTreeMap<EffectPass, RenderEffectDesc>,
}

impl Default for MaterialDesc {
    fn default() -> Self {
        Self {
            effects: [
                (EffectPass::Forward, RenderEffectDesc::default()),
                (EffectPass::Shadow, RenderEffectDesc::OpaqueShadow),
            ]
            .into_iter()
            .collect(),
        }
    }
}

impl Loadable for MaterialDesc {
    type Output = Material;

    async fn load(&self, assets: &AssetCache) -> anyhow::Result<Self::Output> {
        let effects = self.effects.load(assets).await?;

        Ok(Material { effects })
    }
}

declare_resource!(Material, MaterialDesc);

#[derive(Debug, Clone, Resource, Bundle)]
#[resource(derive = [Editable])]
pub struct MaterialBundle {
    #[resource(load)]
    material: Asset<Material>,
}

impl Bundle for MaterialBundle {
    fn mount(&self, entity: &mut flax::EntityBuilder) {
        for (key, effect) in &self.material.effects {
            let component = match key {
                EffectPass::Forward => forward_pass(),
                EffectPass::Transparent => transparent_pass(),
                EffectPass::Shadow => shadow_pass(),
            };

            entity.set(component, effect.clone());
        }
        entity.set(color(), Color::white());
    }
}

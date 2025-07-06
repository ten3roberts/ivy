use std::{any::Any, path::PathBuf};

use anyhow::Context;
use ivy_assets::{AssetCache, AssetPath, loadable::Loadable, meta::AssetPayloadUntyped};
use ivy_scene::editor::registry::EDITABLE_REGISTRY;
use ivy_ui::violet::{
    core::{
        Scope, Widget,
        layout::Align,
        style::{SizeExt, element_danger, surface_danger},
        text::Wrap,
        unit::Unit,
        widget::{LoadingSpinner, SuspenseWidget, bold, col, label},
    },
    futures_signals::signal::Mutable,
    lucide::icons::LUCIDE_TRIANGLE_ALERT,
};

pub struct AssetInspector {
    assets: AssetCache,
    path: PathBuf,
}

impl AssetInspector {
    pub fn new(assets: AssetCache, path: PathBuf) -> Self {
        Self { assets, path }
    }
}

impl Widget for AssetInspector {
    fn mount(self, scope: &mut ivy_ui::violet::core::Scope<'_>) {
        let editor = async move {
            let path = AssetPath::<AssetPayloadUntyped>::new(self.path.canonicalize()?);
            let payload = path.load(&self.assets).await?;

            let value: Mutable<Box<dyn Send + Sync + Any>> =
                Mutable::new(payload.desc().clone_dyn().into_any_sync());

            let editor = EDITABLE_REGISTRY.get_by_name(&payload.meta.type_name);

            let editor = match editor {
                Some(v) => Some(v.create_editor(value)),
                None => None,
            };

            anyhow::Ok((payload, editor))
        };

        SuspenseWidget::new(LoadingSpinner::new("Loading Asset"), async move {
            let editor = editor.await;

            |scope: &mut Scope| match editor {
                Ok((_, Some(editor))) => {
                    editor.mount(scope);
                }
                Ok((payload, None)) => {
                    bold(format!(
                        "No editor available for this asset\n\n{:?}",
                        payload.meta
                    ))
                    .mount(scope);
                }
                Err(e) => {
                    col((
                        label(LUCIDE_TRIANGLE_ALERT)
                            .with_color(surface_danger())
                            .with_font_size(48.0),
                        bold(format!("{e:?}")).with_wrap(Wrap::Word),
                    ))
                    .with_cross_align(Align::Center)
                    .mount(scope);
                }
            }
        })
        .mount(scope);
    }
}

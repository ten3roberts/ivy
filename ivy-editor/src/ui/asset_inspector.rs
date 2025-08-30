use std::{path::PathBuf, time::Duration};

use async_std::stream::StreamExt;
use glam::BVec2;
use ivy_assets::{
    AssetCache, AssetPath,
    loadable::{Loadable, LoadableDyn},
    meta::AssetPayloadUntyped,
};
use ivy_editable::registry::EDITABLE_REGISTRY;
use ivy_ui::violet::{
    core::{
        Scope, Widget,
        layout::Align,
        state::StateExt,
        style::surface_danger,
        text::Wrap,
        time::sleep,
        to_owned,
        widget::{
            LoadingSpinner, ScrollArea, StreamWidget, SuspenseWidget, Throbber, bold, col,
            interactive::base::InteractiveWidget, label, row, subtitle,
        },
    },
    futures_signals::signal::{Mutable, SignalExt},
    lucide::icons::{LUCIDE_CHECK, LUCIDE_TRIANGLE_ALERT},
};

pub struct AssetEditor {
    assets: AssetCache,
    path: PathBuf,
}

impl AssetEditor {
    pub fn new(assets: AssetCache, path: PathBuf) -> Self {
        Self { assets, path }
    }
}

// TODO: move into untyped payload + impl serde
pub struct ErasedAssetDesc {
    desc: Box<dyn LoadableDyn>,
}

impl ErasedAssetDesc {
    pub fn new(desc: Box<dyn LoadableDyn>) -> Self {
        Self { desc }
    }
}

impl Clone for ErasedAssetDesc {
    fn clone(&self) -> Self {
        Self {
            desc: self.desc.clone_dyn(),
        }
    }
}

impl Widget for AssetEditor {
    fn mount(self, scope: &mut ivy_ui::violet::core::Scope<'_>) {
        let editor = async move {
            let path = AssetPath::<AssetPayloadUntyped>::new(self.path.canonicalize()?);
            let payload = path.load(&self.assets).await?;
            anyhow::Ok((path, payload))
        };

        SuspenseWidget::new(LoadingSpinner::new("Loading Asset"), async move {
            async_std::task::sleep(Duration::from_millis(100)).await;
            let editor = editor.await;

            |scope: &mut Scope| match editor {
                Ok((path, payload)) => {
                    let upcast = payload.desc.upcast_boxed_any();
                    let value = Mutable::new(ErasedAssetDesc::new(payload.desc.clone_dyn()));
                    let stream = value.clone().map_value(
                        |v| v.desc.into_any_sync(),
                        move |v| ErasedAssetDesc::new(upcast(v)),
                    );

                    let editor = EDITABLE_REGISTRY.get_by_type((*payload.desc).type_id());

                    let editor = editor.map(|v| v.create_editor(Box::new(stream)));

                    let type_name = payload.meta.type_name.clone();

                    let save_status = value
                        .signal_ref(move |v| AssetPayloadUntyped {
                            meta: payload.meta.clone(),
                            desc: v.desc.clone_dyn(),
                        })
                        .throttle(|| sleep(Duration::from_secs(2)))
                        .to_stream()
                        .skip(1)
                        .map(move |payload| {
                            to_owned!(path);
                            SuspenseWidget::new(Throbber::new(12.0), async move {
                                to_owned!(path);
                                let result = async move {
                                    let payload = payload.serialize_json()?;
                                    async_std::fs::write(path.path(), payload).await?;
                                    sleep(Duration::from_millis(500)).await;
                                    anyhow::Ok(())
                                }
                                .await;

                                |scope: &mut Scope| match result {
                                    Ok(()) => InteractiveWidget::new(label(LUCIDE_CHECK))
                                        .with_tooltip_text("Saved")
                                        .mount(scope),
                                    Err(e) => {
                                        col((
                                            label(LUCIDE_TRIANGLE_ALERT)
                                                .with_color(surface_danger())
                                                .with_font_size(48.0),
                                            bold(format!("Failed to save asset: {e:?}"))
                                                .with_wrap(Wrap::Word),
                                        ))
                                        .with_cross_align(Align::Center)
                                        .mount(scope);
                                    }
                                }
                            })
                        });

                    col((
                        row((subtitle(&type_name), StreamWidget::new(save_status)))
                            .with_cross_align(Align::Center),
                        ScrollArea::new(BVec2::TRUE, editor),
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

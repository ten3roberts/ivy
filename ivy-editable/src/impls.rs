use std::{future::ready, marker::Sized, path::PathBuf, sync::Arc};

use flax::Entity;
use futures::StreamExt;
use glam::{BVec3, Quat, Vec2, Vec3, Vec4};
use itertools::Itertools;
use ivy_assets::{
    AssetCache, AssetPath, Resource, loadable::LoadFromPath,
    services::filesystem_index::FileSystemIndexService,
};
use ordered_float::NotNan;
use violet::{
    self,
    core::{
        Scope, Widget,
        layout::Align,
        state::{StateDuplex, StateExt, StateSink, StateStream, StateStreamRef, StateWrite},
        style::SizeExt,
        to_owned,
        unit::Unit,
        widget::{
            Button, Checkbox, InputBox, LabeledSlider, SignalWidget, StreamWidget, TextInput, card,
            interactive::{
                Dropdown,
                colorpicker::RgbColorPicker,
                overlay::{Overlay, overlay_state},
                select_list::SelectList,
            },
            label, row,
        },
    },
    futures_signals::signal::{Mutable, SignalExt},
    lucide::icons::{LUCIDE_PACKAGE, LUCIDE_PLUS, LUCIDE_TRASH_2},
    palette::{Srgb, Srgba, WithAlpha},
};

use crate::{Editable, EditableWithOpts, EditorOpts};

impl Editable for String {
    const INLINE: bool = true;

    fn create_editor<S: 'static + Send + Sync + StateDuplex<Item = Self>>(
        state: S,
        _assets: &AssetCache,
    ) -> Box<dyn Send + Widget> {
        let state = state.memo(Default::default());
        state.sync_initial();
        Box::new(TextInput::new(state.dedup()))
    }

    fn create_editor_project<S: 'static + Send + Sync + StateStreamRef<Item = Self> + StateWrite>(
        state: S,
        assets: &AssetCache,
    ) -> Box<dyn Send + Widget>
    where
        Self: Sized,
    {
        Self::create_editor(state.project_ref(|v| v, |v| v), assets)
    }
}

impl<T: 'static + Send + Sync + LoadFromPath> Editable for AssetPath<T> {
    const INLINE: bool = true;

    fn create_editor<S: 'static + Send + Sync + StateDuplex<Item = Self>>(
        state: S,
        assets: &AssetCache,
    ) -> Box<dyn Send + Widget>
    where
        Self: Sized,
    {
        #[derive(Clone)]
        struct PathDisplay {
            path: Arc<PathBuf>,
        }

        impl PathDisplay {
            fn new(path: Arc<PathBuf>) -> Self {
                Self { path }
            }
        }

        impl Widget for PathDisplay {
            fn mount(self, scope: &mut Scope<'_>) {
                row((label(LUCIDE_PACKAGE), label(self.path.to_string_lossy()))).mount(scope);
            }
        }

        let filter_name = T::resource_name();
        let index_service = assets.service::<FileSystemIndexService>();

        let items = if let Some(filter_name) = filter_name {
            index_service
                .get_index()
                .into_iter()
                .map(|v| {
                    v.by_type
                        .get(filter_name)
                        .into_iter()
                        .flatten()
                        .map(|v| PathDisplay::new(Arc::clone(v)))
                })
                .flatten()
                .collect_vec()
        } else {
            // Filter by extension, not meta

            index_service
                .get_index()
                .into_iter()
                .map(|v| {
                    T::extensions()
                        .into_iter()
                        .flat_map(|&ext| v.by_extension.get(ext).into_iter().flatten())
                        .map(|v| PathDisplay::new(Arc::clone(v)))
                })
                .flatten()
                .collect_vec()
        };

        Box::new(
            Dropdown::new(
                state.map_value(
                    |v| PathDisplay::new(Arc::new(v.path().to_path_buf())),
                    |v| AssetPath::new(&*v.path),
                ),
                items,
            )
            .searchable(|item| item.path.display().to_string()),
        )
    }

    fn create_editor_project<
        S: 'static + Send + Sync + Clone + StateStreamRef<Item = Self> + StateWrite,
    >(
        state: S,
        assets: &AssetCache,
    ) -> Box<dyn Send + Widget>
    where
        Self: Sized,
    {
        Self::create_editor(state.project_ref(|v| v, |v| v), assets)
    }
}

macro_rules! input_box_impl {
    ($ty: ty) => {
        impl EditableWithOpts for $ty {
            type Range = Self;
            const INLINE: bool = true;

            fn create_editor_opts<S: 'static + Send + Sync + StateDuplex<Item = Self>>(
                state: S,
                opts: crate::EditorOpts<Self>,
                _assets: &AssetCache,
            ) -> Box<dyn Send + Widget> {
                let state = state.memo(Default::default());
                state.sync_initial();
                if let Some(range) = opts.range {
                    Box::new(LabeledSlider::input(state, range.0, range.1))
                } else {
                    Box::new(InputBox::new(state.dedup()))
                }
            }

            fn create_editor_project_opts<
                S: 'static + Send + Sync + StateStreamRef<Item = Self> + StateWrite,
            >(
                state: S,
                opts: crate::EditorOpts<Self>,
                _assets: &AssetCache,
            ) -> Box<dyn Send + Widget>
            where
                Self: Sized,
            {
                let state = state.project_ref(|v| v, |v| v);
                if let Some(range) = opts.range {
                    Box::new(LabeledSlider::input(state, range.0, range.1))
                } else {
                    Box::new(InputBox::new(state))
                }
            }
        }
    };
    ($($ty: ty),+) => {
        $(
            input_box_impl!($ty);
        )+
    };
}

input_box_impl!(i32, u32, i64, u64, i16, u16, i8, u8, f32, f64);

impl Editable for bool {
    const INLINE: bool = true;

    fn create_editor<S: 'static + Send + Sync + StateDuplex<Item = Self>>(
        state: S,
        _assets: &AssetCache,
    ) -> Box<dyn Send + Widget> {
        let state = state.memo(false);
        state.sync_initial();
        Box::new(Checkbox::new(state.dedup()))
    }

    fn create_editor_project<S: 'static + Send + Sync + StateStreamRef<Item = Self> + StateWrite>(
        state: S,
        _assets: &AssetCache,
    ) -> Box<dyn Send + Widget>
    where
        Self: Sized,
    {
        Box::new(Checkbox::new(state.project_ref(|v| v, |v| v)))
    }
}

impl EditableWithOpts for Vec2 {
    type Range = Self;
    const INLINE: bool = true;

    fn create_editor_opts<S: 'static + Send + Sync + StateDuplex<Item = Self>>(
        state: S,
        opts: EditorOpts<Self>,
        assets: &AssetCache,
    ) -> Box<dyn Send + Widget> {
        Self::create_editor_project_opts(Arc::new(state.memo(Default::default())), opts, assets)
    }

    fn create_editor_project_opts<
        S: 'static + Send + Sync + Clone + StateStreamRef<Item = Self> + StateWrite,
    >(
        state: S,
        opts: EditorOpts<Self>,
        assets: &AssetCache,
    ) -> Box<dyn Send + Widget>
    where
        Self: Sized,
    {
        let opts_x = opts.map_range(|v| v.x);
        let opts_y = opts.map_range(|v| v.y);

        let x = state.clone().project_ref(|v| &v.x, |v| &mut v.x);
        let y = state.clone().project_ref(|v| &v.y, |v| &mut v.y);

        Box::new(row((
            f32::create_editor_project_opts(x, opts_x, assets),
            f32::create_editor_project_opts(y, opts_y, assets),
        )))
    }
}

impl EditableWithOpts for Vec3 {
    type Range = Self;
    const INLINE: bool = true;

    fn create_editor_opts<S: 'static + Send + Sync + StateDuplex<Item = Self>>(
        state: S,
        opts: EditorOpts<Self>,
        assets: &AssetCache,
    ) -> Box<dyn Send + Widget> {
        Self::create_editor_project_opts(Arc::new(state.memo(Default::default())), opts, assets)
    }

    fn create_editor_project_opts<
        S: 'static + Send + Sync + Clone + StateStreamRef<Item = Self> + StateWrite,
    >(
        state: S,
        opts: EditorOpts<Self>,
        assets: &AssetCache,
    ) -> Box<dyn Send + Widget>
    where
        Self: Sized,
    {
        let x = state.clone().project_ref(|v| &v.x, |v| &mut v.x);
        let y = state.clone().project_ref(|v| &v.y, |v| &mut v.y);
        let z = state.clone().project_ref(|v| &v.z, |v| &mut v.z);

        let opts_x = opts.map_range(|v| v.x);
        let opts_y = opts.map_range(|v| v.y);
        let opts_z = opts.map_range(|v| v.z);

        Box::new(row((
            f32::create_editor_project_opts(x, opts_x, assets),
            f32::create_editor_project_opts(y, opts_y, assets),
            f32::create_editor_project_opts(z, opts_z, assets),
        )))
    }
}

impl EditableWithOpts for Vec4 {
    type Range = Self;
    const INLINE: bool = true;

    fn create_editor_opts<S: 'static + Send + Sync + StateDuplex<Item = Self>>(
        state: S,
        opts: EditorOpts<Self>,
        assets: &AssetCache,
    ) -> Box<dyn Send + Widget> {
        Self::create_editor_project_opts(Arc::new(state.memo(Default::default())), opts, assets)
    }

    fn create_editor_project_opts<
        S: 'static + Send + Sync + Clone + StateStreamRef<Item = Self> + StateWrite,
    >(
        state: S,
        opts: EditorOpts<Self>,
        assets: &AssetCache,
    ) -> Box<dyn Send + Widget>
    where
        Self: Sized,
    {
        let x = state.clone().project_ref(|v| &v.x, |v| &mut v.x);
        let y = state.clone().project_ref(|v| &v.y, |v| &mut v.y);
        let z = state.clone().project_ref(|v| &v.z, |v| &mut v.z);
        let w = state.clone().project_ref(|v| &v.w, |v| &mut v.w);

        let opts_x = opts.map_range(|v| v.x);
        let opts_y = opts.map_range(|v| v.y);
        let opts_z = opts.map_range(|v| v.z);
        let opts_w = opts.map_range(|v| v.w);

        Box::new(row((
            f32::create_editor_project_opts(x, opts_x, assets),
            f32::create_editor_project_opts(y, opts_y, assets),
            f32::create_editor_project_opts(z, opts_z, assets),
            f32::create_editor_project_opts(w, opts_w, assets),
        )))
    }
}

impl Editable for BVec3 {
    const INLINE: bool = true;

    fn create_editor<S: 'static + Send + Sync + StateDuplex<Item = Self>>(
        state: S,
        assets: &AssetCache,
    ) -> Box<dyn Send + Widget> {
        Self::create_editor_project(Arc::new(state.memo(Default::default())), assets)
    }

    fn create_editor_project<
        S: 'static + Send + Sync + Clone + StateStreamRef<Item = Self> + StateWrite,
    >(
        state: S,
        assets: &AssetCache,
    ) -> Box<dyn Send + Widget>
    where
        Self: Sized,
    {
        let x = state.clone().project_ref(|v| &v.x, |v| &mut v.x);
        let y = state.clone().project_ref(|v| &v.y, |v| &mut v.y);
        let z = state.clone().project_ref(|v| &v.z, |v| &mut v.z);

        Box::new(row((
            bool::create_editor_project(x, assets),
            bool::create_editor_project(y, assets),
            bool::create_editor_project(z, assets),
        )))
    }
}

impl<T: Clone + Editable> Editable for Option<T> {
    const INLINE: bool = T::INLINE;

    fn create_editor<S: 'static + Send + Sync + StateDuplex<Item = Self>>(
        state: S,
        assets: &AssetCache,
    ) -> Box<dyn Send + Widget>
    where
        Self: Sized,
    {
        let state = Arc::new(state);
        let has_value = Arc::new(
            state
                .clone()
                .filter_map(|v| Some(v.is_some()), |_| None)
                .dedup()
                .memo(false),
        );

        has_value.sync_initial();

        let inner_value = Arc::new(state.clone().lower_option());

        to_owned!(assets);
        let inner_value = has_value.stream().map(move |has_value| {
            if has_value {
                Some(T::create_editor(inner_value.clone(), &assets))
            } else {
                None
            }
        });

        let add_remove = has_value.stream().map(move |v| {
            to_owned!(has_value);
            Button::label(if v { LUCIDE_TRASH_2 } else { LUCIDE_PLUS }).on_click(move |_| {
                has_value.send(!v);
            })
        });

        Box::new(row((
            StreamWidget::new(inner_value),
            StreamWidget::new(add_remove),
        )))
    }

    fn create_editor_project<
        S: 'static + Send + Sync + Clone + violet::core::state::StateProjected<Item = Self>,
    >(
        state: S,
        assets: &AssetCache,
    ) -> Box<dyn Send + Widget>
    where
        Self: Sized,
    {
        Self::create_editor(state.project_ref(|v| v, |v| v), assets)
    }
}

impl Editable for Srgb {
    const INLINE: bool = false;

    fn create_editor<S: 'static + Send + Sync + StateDuplex<Item = Self>>(
        state: S,
        _assets: &AssetCache,
    ) -> Box<dyn Send + Widget> {
        Box::new(
            RgbColorPicker::new(state.map_value(|v| v.with_alpha(1.0), |v| v.without_alpha()))
                .enable_alpha(false),
        )
    }

    fn create_editor_project<S: 'static + Send + Sync + StateStreamRef<Item = Self> + StateWrite>(
        state: S,
        assets: &AssetCache,
    ) -> Box<dyn Send + Widget>
    where
        Self: Sized,
    {
        Self::create_editor(state.project_ref(|v| v, |v| v), assets)
    }
}

impl Editable for Srgba {
    const INLINE: bool = false;

    fn create_editor<S: 'static + Send + Sync + StateDuplex<Item = Self>>(
        state: S,
        _assets: &AssetCache,
    ) -> Box<dyn Send + Widget> {
        Box::new(RgbColorPicker::new(state).enable_alpha(true))
    }

    fn create_editor_project<S: 'static + Send + Sync + StateStreamRef<Item = Self> + StateWrite>(
        state: S,
        assets: &AssetCache,
    ) -> Box<dyn Send + Widget>
    where
        Self: Sized,
    {
        Self::create_editor(state.project_ref(|v| v, |v| v), assets)
    }
}

impl Editable for Srgba<u8> {
    const INLINE: bool = false;

    fn create_editor<S: 'static + Send + Sync + StateDuplex<Item = Self>>(
        state: S,
        _assets: &AssetCache,
    ) -> Box<dyn Send + Widget>
    where
        Self: Sized,
    {
        let state = state
            .map_value(
                |v| -> Srgba { v.into_format() },
                |v| Srgba::<u8>::from_format(v),
            )
            .memo(Default::default());
        Box::new(RgbColorPicker::new(state).enable_alpha(true))
    }

    fn create_editor_project<
        S: 'static + Send + Sync + Clone + violet::core::state::StateProjected<Item = Self>,
    >(
        state: S,
        assets: &AssetCache,
    ) -> Box<dyn Send + Widget>
    where
        Self: Sized,
    {
        Self::create_editor(state.project_ref(|v| v, |v| v), assets)
    }
}

impl Editable for Srgb<u8> {
    const INLINE: bool = false;

    fn create_editor<S: 'static + Send + Sync + StateDuplex<Item = Self>>(
        state: S,
        _assets: &AssetCache,
    ) -> Box<dyn Send + Widget>
    where
        Self: Sized,
    {
        let state = state
            .map_value(
                |v| -> Srgba { v.into_format().with_alpha(1.0) },
                |v| Srgba::<u8>::from_format(v).without_alpha(),
            )
            .memo(Default::default());
        Box::new(RgbColorPicker::new(state).enable_alpha(false))
    }

    fn create_editor_project<
        S: 'static + Send + Sync + Clone + violet::core::state::StateProjected<Item = Self>,
    >(
        state: S,
        assets: &AssetCache,
    ) -> Box<dyn Send + Widget>
    where
        Self: Sized,
    {
        Self::create_editor(state.project_ref(|v| v, |v| v), assets)
    }
}

impl Editable for Quat {
    const INLINE: bool = true;

    fn create_editor<S: 'static + Send + Sync + StateDuplex<Item = Self>>(
        state: S,
        _assets: &AssetCache,
    ) -> Box<dyn Send + Widget> {
        let state = Arc::new(
            state
                .map_value(
                    |v| {
                        let (a, b, c) = v.to_euler(glam::EulerRot::YXZ);
                        (to_degrees(a), to_degrees(b), to_degrees(c))
                    },
                    |new_value| {
                        Quat::from_euler(
                            glam::EulerRot::YXZ,
                            to_radians(new_value.0),
                            to_radians(new_value.1),
                            to_radians(new_value.2),
                        )
                    },
                )
                .memo(Default::default()),
        );

        fn to_degrees(radians: f32) -> f32 {
            radians.to_degrees()
        }

        fn to_radians(degrees: f32) -> f32 {
            degrees.to_radians()
        }

        let yaw = state.clone().project_ref(|v| &v.0, |v| &mut v.0);
        let pitch = state.clone().project_ref(|v| &v.1, |v| &mut v.1);
        let roll = state.clone().project_ref(|v| &v.2, |v| &mut v.2);

        Box::new(row((
            InputBox::new(pitch),
            InputBox::new(yaw),
            InputBox::new(roll),
        )))
    }

    fn create_editor_project<S: 'static + Send + Sync + StateStreamRef<Item = Self> + StateWrite>(
        state: S,
        assets: &AssetCache,
    ) -> Box<dyn Send + Widget>
    where
        Self: Sized,
    {
        Self::create_editor(state.project_ref(|v| v, |v| v), assets)
    }
}

#[allow(dead_code)]
pub struct EntityDisplay(pub Entity);

impl Widget for EntityDisplay {
    fn mount(self, _: &mut Scope<'_>) {
        todo!()
        // let (name_tx, name_rx) = oneshot::channel();

        // scope.apply({
        //     move |world| {
        //         if let Ok(entity) = world.entity(self.0) {
        //             let _ = name_tx.send(format!("{entity}"));
        //         } else {
        //             let _ = name_tx.send(format!("{id} <dead>"));
        //         }

        //         Ok(())
        //     }
        // });

        // FutureWidget::new(name_rx.map(|v| v.ok().map(label))).mount(scope);
    }
}

impl Editable for Entity {
    const INLINE: bool = true;

    fn create_editor<S: 'static + Send + Sync + StateDuplex<Item = Self>>(
        state: S,
        _assets: &AssetCache,
    ) -> Box<dyn Send + Widget> {
        let state = Arc::new(state);
        Box::new(
            row((
                StreamWidget::new(state.stream().map(EntityDisplay)),
                Button::label("…").on_click(move |scope| {
                    let state = state.clone();
                    scope
                        .get_context(overlay_state())
                        .open(EntityPicker::new(Box::new(move |id| {
                            state.send(id);
                        })));
                }),
            ))
            .with_cross_align(Align::Center),
        )
    }

    fn create_editor_project<S: 'static + Send + Sync + StateStreamRef<Item = Self> + StateWrite>(
        state: S,
        assets: &AssetCache,
    ) -> Box<dyn Send + Widget>
    where
        Self: Sized,
    {
        Self::create_editor(state.project_ref(|v| v, |v| v), assets)
    }
}

pub struct EntityPicker {
    on_select: Box<dyn Send + Sync + Fn(Entity)>,
}

impl EntityPicker {
    pub fn new(on_select: Box<dyn Send + Sync + Fn(Entity)>) -> Self {
        Self { on_select }
    }
}

impl Overlay for EntityPicker {
    fn create(
        self,
        scope: &mut Scope<'_>,
        token: violet::core::widget::interactive::overlay::OverlayHandle,
    ) {
        let entities = Mutable::new(Vec::new());

        // scope.apply({
        //     to_owned!(entities);
        //     move |world| {
        //         let ids = Query::new(entity_ids()).borrow(world).iter().collect_vec();

        //         entities.set(ids);

        //         Ok(())
        //     }
        // });

        let selection: Mutable<Option<usize>> = Mutable::new(None);

        scope.spawn(selection.signal().to_stream().filter_map(ready).for_each({
            to_owned!(entities);
            move |selection| {
                let id = entities.lock_ref()[selection];
                (self.on_select)(id);
                token.close();
                async {}
            }
        }));

        card(SignalWidget::new(entities.signal_ref(move |v| {
            SelectList::new(
                selection.clone(),
                v.iter().copied().map(EntityDisplay).collect_vec(),
            )
        })))
        .with_max_size(Unit::px2(400.0, 400.0))
        .mount(scope);
    }
}

impl EditableWithOpts for NotNan<f32> {
    type Range = f32;
    const INLINE: bool = true;

    fn create_editor_opts<S: 'static + Send + Sync + StateDuplex<Item = Self>>(
        state: S,
        opts: EditorOpts<f32>,
        assets: &AssetCache,
    ) -> Box<dyn Send + Widget>
    where
        Self: Sized,
    {
        f32::create_editor_opts(
            state.filter_map(|v| Some(*v), |v| NotNan::new(v).ok()),
            opts,
            assets,
        )
    }

    fn create_editor_project_opts<
        S: 'static + Send + Sync + Clone + StateStreamRef<Item = Self> + StateWrite,
    >(
        state: S,
        opts: EditorOpts<f32>,
        assets: &AssetCache,
    ) -> Box<dyn Send + Widget>
    where
        Self: Sized,
    {
        f32::create_editor_opts(
            state
                .project_ref(|v| v, |v| v)
                .filter_map(|v| Some(*v), |v| NotNan::new(v).ok()),
            opts,
            assets,
        )
    }
}

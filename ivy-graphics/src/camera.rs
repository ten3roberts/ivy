//! Contains common camera components

use flax::{component, Debuggable};
use glam::Mat4;
use ivy_assets::Resource;
use ivy_core::{palette::Srgb, Bundle};
use ivy_editable::Editable;
use serde::{de, Deserialize, Serialize};

component! {
    pub projection_matrix: Mat4,
    pub environment_data: EnvironmentData => [Debuggable],
    pub camera_settings: CameraSettings,
}

impl ivy_editable::Editable for CameraSettings {
    const INLINE: bool = false;
    fn create_editor<
        S: 'static
            + Send
            + Sync
            + ivy_editable::__private::violet::core::state::StateDuplex<Item = Self>,
    >(
        value: S,
        assets: &ivy_editable::AssetCache,
    ) -> Box<dyn Send + ivy_editable::__private::violet::core::widget::Widget> {
        use ivy_editable::__private::violet::core::{
            style::SizeExt, StateExt, StateStream, Widget,
        };
        let state = ::std::sync::Arc::new(
            value
                .filter_map(
                    |v| Some((Some(v.projection),)),
                    |(projection,)| {
                        Some(CameraSettings {
                            projection: projection?,
                        })
                    },
                )
                .memo((None,)),
        );
        state.sync_initial();
        let projection = Box::new(
            state
                .clone()
                .project_ref(|v| &v.0, |v| &mut v.0)
                .lower_option(),
        )
            as Box<
                dyn Send
                    + Sync
                    + ivy_editable::__private::violet::core::state::StateDuplex<
                        Item = CameraProjection,
                    >,
            >;
        let assets = assets.clone();
        Box::new(ivy_editable::__private::violet::core::widget::col(
            ({
                let assets = assets.clone();
                move |scope: &mut ivy_editable::__private::violet::core::Scope<'_>| {
                    let assets = &assets;
                    if <CameraProjection as ivy_editable::Editable>::INLINE {
                        ivy_editable::__private::violet::core::widget::row((
                                        ivy_editable::__private::violet::core::widget::Stack::new(
                                                ivy_editable::__private::violet::core::widget::interactive::base::InteractiveWidget::new(
                                                        ivy_editable::__private::violet::core::widget::label(
                                                            "Projection",
                                                        ),
                                                    )
                                                    .with_tooltip_text("CameraProjection"),
                                            )
                                            .with_maximize(
                                                ivy_editable::__private::violet::glam::Vec2::X,
                                            ),
                                        <CameraProjection as ivy_editable::Editable>::create_editor(
                                            projection,
                                            assets,
                                        ),
                                    ))
                                    .with_cross_align(
                                        ivy_editable::__private::violet::core::layout::Align::Center,
                                    )
                                    .mount(scope);
                    } else {
                        ivy_editable::__private::violet::core::widget::Collapsible::new(
                                        ivy_editable::__private::violet::core::widget::interactive::base::InteractiveWidget::new(
                                                ivy_editable::__private::violet::core::widget::label(
                                                    "Projection",
                                                ),
                                            )
                                            .with_tooltip_text("CameraProjection"),
                                        <CameraProjection as ivy_editable::Editable>::create_editor(
                                            projection,
                                            assets,
                                        ),
                                    )
                                    .indent(true)
                                    .mount(scope);
                    }
                }
            }),
        ))
    }
    fn create_editor_project<
        S: 'static
            + Send
            + Sync
            + Clone
            + ivy_editable::__private::violet::core::state::StateStreamRef<Item = Self>
            + ivy_editable::__private::violet::core::state::StateWrite<Item = Self>,
    >(
        state: S,
        assets: &ivy_editable::AssetCache,
    ) -> Box<dyn Send + ivy_editable::__private::violet::core::widget::Widget> {
        use ivy_editable::__private::violet::core::{
            style::SizeExt, StateExt, StateStream, Widget,
        };
        let assets = assets.clone();
        let projection = state
            .clone()
            .project_ref(|v| &v.projection, |v| &mut v.projection);
        Box::new(ivy_editable::__private::violet::core::widget::col(
            ({
                let assets = assets.clone();
                move |scope: &mut ivy_editable::__private::violet::core::Scope<'_>| {
                    let assets = &assets;
                    if <CameraProjection as ivy_editable::Editable>::INLINE {
                        ivy_editable::__private::violet::core::widget::row((
                                        ivy_editable::__private::violet::core::widget::Stack::new(
                                                ivy_editable::__private::violet::core::widget::interactive::base::InteractiveWidget::new(
                                                        ivy_editable::__private::violet::core::widget::label(
                                                            "Projection",
                                                        ),
                                                    )
                                                    .with_tooltip_text("CameraProjection"),
                                            )
                                            .with_maximize(
                                                ivy_editable::__private::violet::glam::Vec2::X,
                                            ),
                                        <CameraProjection as ivy_editable::Editable>::create_editor_project(
                                            projection,
                                            assets,
                                        ),
                                    ))
                                    .with_cross_align(
                                        ivy_editable::__private::violet::core::layout::Align::Center,
                                    )
                                    .mount(scope);
                    } else {
                        ivy_editable::__private::violet::core::widget::Collapsible::new(
                                        ivy_editable::__private::violet::core::widget::interactive::base::InteractiveWidget::new(
                                                ivy_editable::__private::violet::core::widget::label(
                                                    "Projection",
                                                ),
                                            )
                                            .with_tooltip_text("CameraProjection"),
                                        <CameraProjection as ivy_editable::Editable>::create_editor_project(
                                            projection,
                                            assets,
                                        ),
                                    )
                                    .indent(true)
                                    .mount(scope);
                    }
                }
            }),
        ))
    }
}
#[derive(Debug, Default, Clone, Copy, Resource)]
#[resource(derive = [])]
pub struct CameraBundle {
    camera_settings: CameraSettings,
    environment_data: EnvironmentData,
}

impl CameraBundle {
    pub fn new(camera_settings: CameraSettings, environment_data: EnvironmentData) -> Self {
        Self {
            camera_settings,
            environment_data,
        }
    }
}

impl Bundle for CameraBundle {
    fn mount(&self, entity: &mut flax::EntityBuilder) {
        entity
            .set(camera_settings(), self.camera_settings)
            .set(environment_data(), self.environment_data)
            .set_default(projection_matrix());
    }
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CameraSettings {
    projection: CameraProjection,
}

impl Default for CameraSettings {
    fn default() -> Self {
        Self {
            projection: CameraProjection::perspective(1.2, 0.1, 1000.0),
        }
    }
}

impl CameraSettings {
    pub fn new(projection: CameraProjection) -> Self {
        Self { projection }
    }

    pub fn projection(&self) -> &CameraProjection {
        &self.projection
    }

    pub fn projection_mut(&mut self) -> &mut CameraProjection {
        &mut self.projection
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Editable)]
pub enum CameraProjection {
    Perspective {
        fov_y: f32,
        near: f32,
        far: f32,
    },
    Orthographic {
        left: f32,
        right: f32,
        bottom: f32,
        top: f32,
        near: f32,
        far: f32,
    },
}

impl CameraProjection {
    pub fn perspective(fov_y: f32, near: f32, far: f32) -> Self {
        Self::Perspective { fov_y, near, far }
    }

    pub fn orthographic(left: f32, right: f32, bottom: f32, top: f32, near: f32, far: f32) -> Self {
        Self::Orthographic {
            left,
            right,
            bottom,
            top,
            near,
            far,
        }
    }

    pub fn create_projection_matrix(&self, aspect: f32) -> Mat4 {
        match self {
            CameraProjection::Perspective { fov_y, near, far } => {
                Mat4::perspective_rh(*fov_y, aspect, *near, *far)
            }
            CameraProjection::Orthographic {
                left,
                right,
                bottom,
                top,
                near,
                far,
            } => Mat4::orthographic_rh(*left, *right, *bottom, *top, *near, *far),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Copy, serde::Serialize, serde::Deserialize, Editable)]
pub struct EnvironmentData {
    pub fog_color: Srgb,
    #[editable(range(0.0, 0.01))]
    pub fog_density: f32,
    #[editable(range(0.0, 1.0))]
    pub fog_blend: f32,
    #[editable(range(-100.0, 100.0))]
    pub fog_height: f32,
}

impl EnvironmentData {
    pub fn new(fog_color: Srgb, fog_density: f32, fog_blend: f32, fog_height: f32) -> Self {
        Self {
            fog_color,
            fog_density,
            fog_blend,
            fog_height,
        }
    }
}

impl Default for EnvironmentData {
    fn default() -> Self {
        Self {
            fog_color: Srgb::new(0.3, 0.3, 0.5),
            fog_density: 0.005,
            fog_blend: 0.0,
            fog_height: -1.0,
        }
    }
}

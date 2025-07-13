use futures::StreamExt;
use ivy_editable::Editable;
use violet::{
    core::{state::StateStream, widget::StreamWidget},
    futures_signals::signal::Mutable,
};

#[derive(Clone, Debug, PartialEq, Eq, Editable)]
pub struct MyStruct {
    pub name: String,
    pub value: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MyEnum {
    Variant1 { name: String, value: i32 },
}

impl ivy_editable::Editable for MyEnum {
    const INLINE: bool = false;
    fn create_editor<
        S: 'static
            + Send
            + Sync
            + ivy_editable::__private::violet::core::state::StateDuplex<Item = Self>,
    >(
        state: S,
    ) -> Box<dyn Send + ivy_editable::__private::violet::core::widget::Widget> {
        use ::std::sync::Arc;
        use ivy_editable::__private::violet::core::state::StateExt;
        use ivy_editable::__private::violet::core::style::SizeExt;
        use ivy_editable::__private::violet::core::widget::{Selectable, Widget, col, label, row};
        let state = ::std::sync::Arc::new(state);
        let discriminant = Arc::new(
            state
                .clone()
                .filter_map(
                    |v| {
                        Some(Some(match v {
                            MyEnum::Variant1 { .. } => "Variant1",
                        }))
                    },
                    |_| None,
                )
                .memo(None)
                .dedup()
                .lower_option(),
        );
        let kind_selection = row(Selectable::new_value(
            label("Variant1"),
            discriminant.clone(),
            "Variant1",
        ));
        let value_editor = discriminant.stream().map(move |disc| match disc {
            "Variant1" => {
                let state = Arc::new(state.clone().filter_map(
                    |v| {
                        if let Self::Variant1 { name, value } = v {
                            Some((name, value))
                        } else {
                            None
                        }
                    },
                    |(name, value)| Some(Self::Variant1 { name, value }),
                ))
                .memo(Default::default());
                Box::new(ivy_editable::__private::violet::core::widget::col(()))
                    as Box<dyn Send + Widget>
            }
            _ => todo!(),
        });
        Box::new(col((kind_selection, StreamWidget::new(value_editor))))
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
    ) -> Box<dyn Send + ivy_editable::__private::violet::core::widget::Widget> {
        use ivy_editable::__private::violet::core::state::StateExt;
        use ivy_editable::__private::violet::core::style::SizeExt;
        use ivy_editable::__private::violet::core::widget::Widget;
        todo!()
    }
}

#[test]
fn editable_derive_struct() {
    let s = Mutable::new(MyStruct {
        name: "Test".to_string(),
        value: 42,
    });

    let editor = Editable::create_editor(s.clone());
}

#[test]
fn editable_derive_enum() {
    let s = Mutable::new(MyEnum::Variant1 {
        name: "Test".to_string(),
        value: 42,
    });

    let editor = Editable::create_editor(s.clone());
}

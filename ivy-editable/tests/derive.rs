use futures::StreamExt;
use ivy_editable::Editable;
use violet::{
    core::state::StateStream,
    futures_signals::signal::Mutable,
};

#[derive(Clone, Debug, PartialEq, Eq, Editable)]
pub struct MyStruct {
    pub name: String,
    pub value: i32,
}

#[derive(Clone, Debug, PartialEq, Eq, Editable)]
pub enum MyEnum {
    Variant1 { name: String, value: i32 },
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

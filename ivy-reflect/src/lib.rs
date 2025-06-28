pub enum TypeLayout {
    Primitive(PrimitiveLayout),
    Struct(StructLayout),
    Enum(EnumLayout),
}

pub struct StructLayout {
    fields: Vec<(FieldName, TypeLayout)>,
}

impl StructLayout {
    pub fn new(fields: Vec<(FieldName, TypeLayout)>) -> Self {
        Self { fields }
    }
}

pub struct EnumLayout {
    variants: Vec<(String, TypeLayout)>,
}

impl EnumLayout {
    pub fn new(variants: Vec<(String, TypeLayout)>) -> Self {
        Self { variants }
    }
}

pub struct PrimitiveValue {
    type_name: String,
}

type FieldName = String;

trait Reflect {
    fn as_struct(&self) -> Option<&dyn ReflectStruct> {
        None
    }
}

trait ReflectStruct {
    fn field(&self, field: &str) -> &dyn Reflect;
    fn field_mut(&mut self, field: &str) -> &mut dyn Reflect;
}

impl Reflect for i32 {}
impl Reflect for f32 {}
impl Reflect for usize {}

impl Reflect for String {}

use hecs::{Component, EntityBuilderClone};

pub trait Template: Component {
    fn builder(&self) -> EntityBuilderClone;
}

impl<F: Fn() -> EntityBuilderClone + Component> Template for F {
    fn builder(&self) -> EntityBuilderClone {
        (self)()
    }
}

impl Template for EntityBuilderClone {
    fn builder(&self) -> EntityBuilderClone {
        self.clone()
    }
}

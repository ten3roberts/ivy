use flax::component;

use crate::InputState;

component! {
    // pub on_input_event: Box<dyn Send + Sync + Fn(EntityRef, InputEvent)>,
    pub input_state: InputState,
}

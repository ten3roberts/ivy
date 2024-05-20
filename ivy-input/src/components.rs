use flax::{component, EntityRef};

use crate::{InputEvent, InputState};

component! {
    // pub on_input_event: Box<dyn Send + Sync + Fn(EntityRef, InputEvent)>,
    pub input_state: InputState,
}

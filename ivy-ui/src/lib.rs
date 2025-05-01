use std::{cell::RefCell, rc::Rc};

use violet::wgpu::app::AppInstance;

pub mod components;
pub mod image;
pub mod layer;
pub mod node;
pub mod streamed;

pub type SharedUiInstance = Rc<RefCell<AppInstance>>;

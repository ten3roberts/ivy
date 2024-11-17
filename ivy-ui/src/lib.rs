use std::{cell::RefCell, rc::Rc};
use violet::wgpu::app::AppInstance;

pub mod layer;
pub mod node;
pub mod components;
pub mod image;

pub type SharedUiInstance = Rc<RefCell<AppInstance>>;

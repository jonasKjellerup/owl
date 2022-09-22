use std::{sync::RwLock, rc::Rc, collections::HashMap};
use smithay_client_toolkit::reexports::calloop;
use modules::Module;

pub mod error;
pub mod modules;
pub mod wayland;

pub use modules::prelude as module_prelude;

// TODO check whether `Box<dyn Module>` can be substituted with `dyn Module`
pub type Modules = Rc<RwLock<HashMap<&'static str, Box<dyn Module>>>>;

pub struct UpdateHandle(bool);

impl UpdateHandle {
    pub fn new() -> Self {
        UpdateHandle(true)
    }

    pub fn update(&mut self) {
        self.0 = true;
    }
}

pub struct SharedLoopData {
    pub update_handle: UpdateHandle,
    pub modules: Modules,
}

pub type EventLoop<'l> = calloop::EventLoop<'l, SharedLoopData>;
pub type LoopHandle<'l> = calloop::LoopHandle<'l, SharedLoopData>;

impl SharedLoopData {
    pub fn update(&mut self) { self.update_handle.update(); }
}
//! Core facilities for implementing modules.
//! Builtin modules are organised as submodules of this module.
//!
//! # Module structure
//! Modules provide two main functionalities: widgets and a write function.
//! These are provided by means of implementing the `Module` trait.
//!
//! The write function is a means of exposing textual information to widgets
//! and or other modules. One such example is the text widget, which allows
//! users to display and interpolate these exposed values as text in their bar.

use std::collections::HashMap;
use std::fmt::{Debug, Display, format, Formatter, write};
use std::io;
use std::path::Path;
use cairo::glib::Source;
use smithay_client_toolkit::reexports::calloop::EventSource;
use crate::calloop::{PostAction, RegistrationToken};
use crate::{LoopHandle, SharedLoopData};
use crate::modules::sway::SwayModule;
use crate::error::{Error, Kind, Severity, Result};

pub mod battery;
pub mod sway;



// TODO: provide an interface that allows modules to provide and use
//       their own internal error type in manner that integrates
//       with the core loop in such a manner that fatal error can be caught
//       and handled by the core loop.
//       -
//       The idea is to provide this via the `ModuleError` trait
//       that individual modules can implement for their internal error types.
//       -
//       The current concern is figuring out how to handle the errors
//       at the site of the interface between the core loop and the module code
//       without adding unnecessary complexity or boxing.
//       -
//       Why?: because currently each module is relying on the same result type,
//             which makes the use of the ? slightly difficult since we loose
//             information about the source.
//       -
//       An idea: Create an EventSource that functions as a wrapper around another
//                another EventSource such that we can automate logging of error results.

/// Trait for describing errors produced by modules.
pub trait ModuleError {
    fn module_name(&self) -> &'static str;

    fn severity(&self) -> Severity;

    fn into_msg(self) -> String;
}

impl<T: ModuleError> From<T> for Error {
    fn from(err: T) -> Self {
        Self {
            kind: Kind::ModuleError(err.severity()),
            msg: Some(err.into_msg()),
        }
    }
}

pub mod prelude {
    pub use crate::error::{
        Result, Error, Kind, Severity,
    };
}

/// Describes a widget component of a module.
///
pub trait Widget {

    /// Configures the widget based on user supplied configuration data.
    /// If a fatal error is encountered during configuration of the module
    /// the module will be dropped.
    fn configure(&mut self) -> Result<()>;

    /// Computes the space that the widget will take up. This is called
    /// whenever the widget is to be updated and re-rendered. A consecutive call
    /// to the `Widget::draw` function must never draw outside the bounds given
    /// by this functions.
    fn compute_dimensions(&self) -> (u32, u32);

    fn draw(&self);

}

type WidgetBuilder = fn() -> Box<dyn Widget>;

pub trait Named {
    const NAME: &'static str;
}

pub trait Module {
    fn write(&self, field: &str, dst: &mut String) -> Result<bool>;

    fn register_hooks(&mut self, _handle: LoopHandle) -> Result<()> { Ok(()) }

    fn unregister_hooks(&mut self, _handle: LoopHandle) -> Result<()> { Ok(()) }

    fn register_widgets(&mut self, _register: WidgetRegister) -> Result<()> { Ok(()) }
}

pub struct ModuleInfo {
    name: &'static str,
    module: Box<dyn Module>,
}

pub struct ModuleHandle<'l>(&'l mut ModuleInfo, LoopHandle<'l>);

impl<'l> ModuleHandle<'l> {
    fn register_event_source<S, F, E>(&mut self, source: S, mut callback: F) -> Result<()>
        where
            S: EventSource<Ret=io::Result<PostAction>> + 'l,
            F: FnMut(S::Event, &mut S::Metadata, &mut SharedLoopData) -> std::result::Result<(), E> + 'l,
            E: ModuleError,
    {
        self.1
            .insert_source(source, move |event, metadata, data| {
                Ok(if let Err(err) = callback(event, metadata, data) {
                    match err.severity() {
                        Severity::Warning => PostAction::Continue,
                        Severity::Fatal => PostAction::Remove,
                    }
                } else {
                    PostAction::Continue
                })
            })
            .map(|_| ())
            .map_err(|err| Error::new(Kind::ModuleError(Severity::Fatal))
                .with_msg(format!("Unable to insert event source: {}", err.to_string())))
    }
}

pub struct WidgetRegister<'l>(&'static str, &'l mut ModuleRegistry);

impl<'l> WidgetRegister<'l> {
    pub fn register_widget(&mut self, name: &'static str, builder: WidgetBuilder) {
        self.1.widgets.insert((self.0, name), builder);
    }
}

/// Structure for managing all loaded modules and their associated widgets.1
pub struct ModuleRegistry {
    modules: HashMap<&'static str, ModuleInfo>,
    widgets: HashMap<(&'static str, &'static str), WidgetBuilder>,
}

impl ModuleRegistry {
    /// Instantiates a new, empty, module registry.
    pub fn new() -> Self {
        Self { modules: HashMap::new(), widgets: HashMap::new() }
    }

    pub fn register<M: Module + Named + 'static>(&mut self, mut module: M) -> Result<()> {
        module.register_widgets(WidgetRegister(M::NAME, self))?;
        self.modules.insert(M::NAME, ModuleInfo {
            name: M::NAME,
            module: Box::new(module),
        });
        Ok(())
    }

    pub fn register_widget<M: Module + Named>(&mut self, name: &'static str, builder: WidgetBuilder) -> Result<()> {
        let id = (M::NAME, name);
        self.widgets.insert(id, builder);
        Ok(())
    }
}

pub fn build_module_registry() -> Result<()> {
    let mut registry = ModuleRegistry::new();

    SwayModule::register(&mut registry)?;

    Ok(())
}
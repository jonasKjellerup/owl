use std::fmt::{Display, Formatter};
use std::io;

/// Similar in concept to the currently unstable try_block feature,
/// this function is intended to provide a new scope in which the `?`
/// operator can be used. This allows one to handle multiple fallible operations
/// in one place, rather than doing so individually.
pub fn gather_err<V, E, F>(mut f: F) -> std::result::Result<V, E>
    where F: FnMut() -> std::result::Result<V, E>
{
    f()
}

/// Describes the severity of an error produced by a module.
/// We classify all module errors as being either a warning or as
/// being fatal.
///
/// Warnings are to be used for providing diagnostic information about
/// potential or actual issues with the module as a means for troubleshooting.
/// A warning must always be fully recoverable.
///
/// Fatal errors are errors that the module is unable to recover from.
/// When a fatal error is encountered the module will be unloaded.
#[derive(Debug, Copy, Clone)]
pub enum Severity {
    Warning,
    Fatal,
}

#[derive(Debug)]
pub enum Kind {
    IoError,
    UdevError,
    WaylandError,
    ModuleError(Severity),
    Generic(Box<dyn std::error::Error>),
}

#[derive(Debug)]
pub struct Error {
    pub(crate) kind: Kind,
    pub(crate) msg: Option<String>,
}

impl Error {
    pub fn new(kind: Kind) -> Self {
        Error {
            kind,
            msg: None,
        }
    }

    pub fn with_msg<T: Into<String>>(mut self, msg: T) -> Self {
        self.msg = Some(msg.into());
        self
    }

    pub fn udev_invalid_device_attribute(device: &str, attr: &str) -> Self {
        Self {
            kind: Kind::UdevError,
            msg: Some(format!("Device attribute `{}` of device `{}` is missing or invalid.", device, attr)),
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self.msg {
            Some(msg) => write!(f, "{:?} encountered: `{}`", self.kind, msg),
            None => write!(f, "{:?} encountered. Cause unknown.", self.kind)
        }
    }
}

impl std::error::Error for Error {}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Self {
            kind: Kind::IoError,
            msg: Some(err.to_string()),
        }
    }
}

impl From<Box<dyn std::error::Error>> for Error {
    fn from(err: Box<dyn std::error::Error>) -> Self {
        Self { kind: Kind::Generic(err), msg: None }
    }
}

pub type Result<T> = std::result::Result<T, Error>;
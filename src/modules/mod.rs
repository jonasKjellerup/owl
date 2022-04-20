use std::collections::HashMap;
use std::fmt::{Debug, Display, format, Formatter, write};
use std::path::Path;

pub mod battery;

#[derive(Debug)]
pub enum ErrKind {
    IoError,
    UdevError,
}

#[derive(Debug)]
pub struct Error {
    kind: ErrKind,
    msg: Option<String>,
}

impl Error {
    fn udev_invalid_device_attribute(device: &str, attr: &str) -> Self {
        Self {
            kind: ErrKind::UdevError,
            msg: Some(format!("Device attribute `{}` of device `{}` is missing or invalid.", device, attr)),
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self.msg {
            Some(msg) => write!(f, "{:?} encountered: `{}`", self.kind, msg),
            None => write!(f, "{:?} encounterde. Cause unknown.", self.kind)
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Self {
            kind: ErrKind::IoError,
            msg: Some(err.to_string()),
        }
    }
}

type Result<T> = std::result::Result<T, Error>;

pub struct Fragment {

}

pub trait Module: Sized {
    const NAME: &'static str;

    fn init() -> Result<Self>;

    fn write(&self, field: &str, dst: &mut String) -> Result<()>;

}
use std::{env, io};
use std::cmp::max;
use std::mem::size_of;
use std::os::unix::net::UnixStream;
use std::io::{Cursor, Read, Seek, SeekFrom, Write};
use log::{debug, trace};
use crate::modules::{Error, Severity, Result,};
use crate::error::gather_err;
use serde::{Deserialize};
use byteorder::{ReadBytesExt, WriteBytesExt, NativeEndian};
use serde_json as json;
use crate::modules::Kind::{IoError, ModuleError};

const MSG_RUN_COMMAND: u32 = 0;
const MSG_GET_WORKSPACES: u32 = 1;
const MSG_SUBSCRIBE: u32 = 2;
const MSG_GET_TREE: u32 = 4;
const MSG_GET_BINDING_STATE: u32 = 12;

pub const EVENT_WORKSPACE: u32 = 0x80000000;
pub const EVENT_MODE: u32 = 0x80000002;
pub const EVENT_WINDOW: u32 = 0x80000003;
pub const EVENT_SHUTDOWN: u32 = 0x80000006;

const MAGIC_WORD: &'static [u8] = b"i3-ipc";
const HEADER_LEN: usize = 8 + MAGIC_WORD.len();

pub struct IpcStream {
    socket: UnixStream,
    buffer: Vec<u8>,
}

impl IpcStream {
    /// Reads exactly n bytes into the internal buffer.
    /// If fewer bytes are read, then an IoError with the given
    /// err_msg will be returned.
    ///
    /// The internal buffer is automatically expanded as needed.
    fn read_n(&mut self, n: usize, err_msg: &str) -> Result<()> {
        // TODO: Ideally we want to use `ReadBuf` instead once it has been stabilized.
        //       Current solution requires unnecessary writes for initializing memory before read.
        self.buffer.resize(n, 0);
        let count = self.socket.read(&mut self.buffer)?;
        if count < n {
            return Err(Error::new(IoError).with_msg(err_msg));
        }

        Ok(())
    }

    pub fn read_message(&mut self) -> Result<Message> {
        self.read_n(HEADER_LEN, "Invalid ipc message header format.")?;

        let expected_len: usize;
        let msg_type: u32;
        {
            let mut cursor = Cursor::new(&self.buffer);
            cursor.seek(SeekFrom::Start(MAGIC_WORD.len() as u64))?;
            expected_len = cursor.read_u32::<NativeEndian>()? as usize;
            msg_type = cursor.read_u32::<NativeEndian>()?;
        }

        self.read_n(expected_len, "Ipc message body too short.")?;
        let payload = &self.buffer[..expected_len];

        let s = String::from_utf8_lossy(payload);
        trace!("Ipc message received: id = {:#x} ; msg = {}", msg_type, s);
        let msg_body: json::Value = json::from_slice(payload)
            .map_err(|e| {
                trace!("Error parsing ipc message body: {}", e);
                Error::new(IoError).with_msg("Error parsing ipc message body.")
            })?;

        Ok(Message {
            kind: msg_type,
            body: msg_body,
        })
    }

    /// Finds the sway/i3 IPC socket path and attempts
    /// to open the socket and subscribe.
    ///
    /// The function does not wait for a response to
    /// the subscribe request before returning.
    pub fn open_and_subscribe(subscriptions: &[&str]) -> Result<Self> {
        let socket_path =
            env::var("SWAYSOCK")
                .or(env::var("I3SOCK"))
                .map_err(|_|
                    Error::new(IoError)
                        .with_msg("Unable to find sway/i3 socket ipc path.")
                )?;
        let mut socket: UnixStream = UnixStream::connect(&socket_path)?;

        gather_err(|| {
            let body = json::ser::to_string(&subscriptions)
                .map_err(<serde_json::Error as Into<io::Error>>::into)?;
            socket.write(MAGIC_WORD)?;
            socket.write_u32::<NativeEndian>(body.len() as u32)?;
            socket.write_u32::<NativeEndian>(MSG_SUBSCRIBE)?;
            socket.write(body.as_bytes()).map(|_| ())
        }).map_err(|_| Error::new(IoError).with_msg("Unable to write to ipc socket."))?;

        let mut stream = Self {
            socket,
            buffer: Vec::with_capacity(2048),
        };

        let Message { kind, body } = stream.read_message()?;

        if kind != MSG_SUBSCRIBE {
            Err(Error::new(IoError).with_msg("Unexpected response to sway-ipc subscription attempt."))
        } else {
            body.get("success")
                .ok_or_else(|| {
                    trace!("Expected JSON property `success` missing in sway IPC subscription response.");
                    Error::new(IoError).with_msg("Invalid response to sway-ipc subscription attempt.")
                })
                .and_then(|value| {
                    value.as_bool()
                        .ok_or_else(|| {
                            trace!("Expected JSON property `success` to be a boolean.");
                            Error::new(IoError)
                                .with_msg("Invalid response to sway-ipc subscription attempt.")
                        })
                })
                .and_then(|value| {
                    if value {
                        Ok(stream)
                    } else {
                        trace!("Success state in sway IPC subscription response is false.");
                        Err(Error::new(IoError).with_msg("Unable to subscribe to sway-ipc events."))
                    }
                })
        }
    }
}

#[derive(Debug)]
pub struct Message {
    pub kind: u32,
    pub body: json::Value,
}

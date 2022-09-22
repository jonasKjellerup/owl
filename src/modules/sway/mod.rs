//! Module for displaying status information of the sway/i3 window managers.
pub mod ipc; // TODO use other pub() modifier

use std::os::unix::net::UnixStream;
use std::io::{self, Read, Write};
use crate::calloop::generic::{Fd, Generic};
use crate::calloop::{Interest, Mode, PostAction, RegistrationToken, Readiness};
use crate::{Module, LoopHandle, SharedLoopData, modules::Result};
use byteorder::{ReadBytesExt, WriteBytesExt, NativeEndian};
use crate::modules::{ModuleRegistry, Named};
use crate::error::gather_err;

const MODULE_NAME: &'static str = "core:sway";

pub struct SwayModule {
    reg_token: Option<RegistrationToken>,
    focused_view_name: Option<String>,
    workspaces: Vec<()>,
    connection_established: bool,
}

impl SwayModule {

    pub fn register(registry: &mut ModuleRegistry) -> Result<()> {
        let instance = SwayModule {
            reg_token: None,
            focused_view_name: None,
            workspaces: Vec::new(),
            connection_established: false,
        };

        registry.register(instance)

    }

    fn handle_event(
        ready_state: Readiness,
        socket: &mut UnixStream,
        shared: &mut SharedLoopData,
    ) -> io::Result<PostAction>
    {
        Ok(PostAction::Continue)

        /*let result: Result<PostAction> = gather_err(|| {
            if let Readiness { readable: true, writable: true, .. } = ready_state {
                let module_state = {
                    let mut modules = shared.modules.write().unwrap();
                    let m = modules.get_mut(MODULE_NAME).unwrap();
                    // TODO handle this in a less ugly manner
                    unsafe { &mut *(m.as_mut() as *mut dyn Module as *mut SwayModule) }
                };
                let read_count = socket.read(&mut shared.buffer)?;
                // TODO: Delegate handling of module errors to function
                //      (we can't propagate the below error since it is not of type io::Error)
                let msg = ipc::Reply::decode(&shared.buffer[..read_count])?;
                // TODO filter msgs that we don't care about

                let payload = msg.msg_payload()?;
                match payload {
                    IpcMessage::WorkspaceEvent(ipc::WorkspaceEvent { change, current: Some(info), .. })
                    if change == "init" => {
                        module_state.workspaces.push(info);
                        shared.update();
                    }

                    IpcMessage::WorkspaceEvent(ipc::WorkspaceEvent { change, current: Some(info), .. })
                    if change == "empty" => {
                        let mut i = 0;
                        while i < module_state.workspaces.len() {
                            if module_state.workspaces[i].name == info.name {
                                module_state.workspaces.remove(i);
                                break;
                            }
                            i += 1;
                        }
                        shared.update();
                    }

                    IpcMessage::ShutdownEvent(ipc::ShutdownEvent { reason }) => {}
                    _ => (),
                }


                Ok(PostAction::Continue)
            } else { Ok(PostAction::Continue) }
        });

        // TODO log any errors before disabling the event

        result.or(Ok(PostAction::Disable))*/
    }
}

impl Module for SwayModule {
    fn write(&self, field: &str, dst: &mut String) -> Result<bool> {
        let is_valid_field = match field {
            "focused_view_name" => {
                if let Some(name) = &self.focused_view_name {
                    dst.push_str(name);
                }
                true
            }
            _ => false,
        };
        Ok(is_valid_field)
    }

    /*
    fn register_hooks(&mut self, handle: LoopHandle) -> Result<()> {
        let socket = ipc::start_and_subscribe()?;

        self.reg_token = Some(handle.insert_source(
            Generic::new(socket, Interest::BOTH, Mode::Edge),
            Self::handle_event,
        ).unwrap());

        Ok(())
    }

    fn unregister_hooks(&mut self, handle: LoopHandle) -> Result<()> {
        if let Some(token) = self.reg_token.take() {
            handle.remove(token);
        }
        Ok(())
    }*/
}

impl Named for SwayModule {
    const NAME: &'static str = MODULE_NAME;
}
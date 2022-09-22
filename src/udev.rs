use udev::MonitorSocket;
use crate::calloop::{EventSource, PostAction, Readiness, TokenFactory, Interest, Mode, generic::Generic, LoopHandle, InsertError};
use crate::calloop::generic::Fd;

struct Hook(Generic<MonitorSocket>);

impl Hook {
    fn new() -> std::io::Result<Self> {
        let socket = udev::MonitorBuilder::new()
            .and_then(|builder| builder.match_subsystem_devtype("power_supply", "BAT"))
            .and_then(|builder| builder.listen())?;

        Ok(Hook(Generic::new(socket, Interest::READ, Mode::Edge)))
    }
}
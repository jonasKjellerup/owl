use std::io::Read;
use log::{debug, error, info};
use owl::modules::sway::ipc;
use owl::modules::sway::ipc::Message;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Trace)
        .init();
    debug!("Debug logging enabled");

    info!("Connecting to sway ipc.");
    let mut stream = ipc::IpcStream::open_and_subscribe(&["window", "workspace", "mode", "shutdown"])?;
    loop {
        let msg = stream.read_message();
        if let Ok(Message { kind, body }) = msg {
            match kind {
                ipc::EVENT_SHUTDOWN => {
                    let reason = body.get("change")
                        .and_then(|v| v.as_str())
                        .unwrap_or("<no reason provided>");
                    info!("Shutdown event received. Reason: {}", reason);
                    return Ok(());
                },
                _ => info!("Ipc message received: kind = {:#x}, body = {}", kind, body),
            }
        } else {
            error!("Ipc message error encountered: {:?}", msg.unwrap_err());
        }

    }
}
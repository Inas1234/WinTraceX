use crate::model::event::Event;
use crate::model::ipc::TRACE_UDP_BIND_ADDR;
use std::net::UdpSocket;
use std::sync::mpsc::Sender;

pub fn start_udp_event_listener(sender: Sender<Event>) -> Result<(), String> {
    let socket =
        UdpSocket::bind(TRACE_UDP_BIND_ADDR).map_err(|e| format!("UDP bind failed: {e}"))?;

    std::thread::Builder::new()
        .name("udp-event-listener".to_owned())
        .spawn(move || {
            let mut buffer = [0u8; 8192];
            loop {
                let Ok((size, _peer)) = socket.recv_from(&mut buffer) else {
                    continue;
                };

                let Ok(event) = serde_json::from_slice::<Event>(&buffer[..size]) else {
                    continue;
                };

                let _ = sender.send(event);
            }
        })
        .map_err(|e| format!("Failed to spawn UDP listener thread: {e}"))?;

    Ok(())
}

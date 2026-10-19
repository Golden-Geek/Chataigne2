use super::*;
use std::{
    collections::VecDeque,
    io::{self, Cursor},
};
use tokio_tungstenite::tungstenite::protocol::Role;

struct Wire {
    input: Cursor<Vec<u8>>,
    output: Vec<u8>,
    actions: VecDeque<io::Result<usize>>,
}
impl Read for Wire {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.input.read(bytes)
    }
}
impl Write for Wire {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let count = self.actions.pop_front().unwrap_or(Ok(bytes.len()))?.min(bytes.len());
        self.output.extend_from_slice(&bytes[..count]);
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn encoded_frame_is_submitted_once_and_only_its_remaining_wire_bytes_are_flushed() {
    let payload = vec![7; 100_000];
    let wire = Wire {
        input: Cursor::new(vec![]),
        output: vec![],
        actions: VecDeque::from([
            Ok(7),
            Err(io::ErrorKind::WouldBlock.into()),
            Err(io::ErrorKind::WouldBlock.into()),
            Ok(11),
        ]),
    };
    let limits = crate::app::module::common::streaming::websocket::websocket_config().write_buffer_size(0);
    let mut socket = WebSocket::from_raw_socket(wire, Role::Client, Some(limits));
    let mut command = Outbound::new(StreamingSendFrameKind::Binary, &payload, 4, true).unwrap();
    command.prepare().unwrap();
    assert!(!command.write_turn(&mut socket).unwrap());
    assert!(command.submitted);
    assert!(!command.write_turn(&mut socket).unwrap());
    assert!(command.write_turn(&mut socket).unwrap());
    let wire = socket.get_ref().output.clone();
    let mut server = WebSocket::from_raw_socket(
        Wire {
            input: Cursor::new(wire),
            output: vec![],
            actions: VecDeque::new(),
        },
        Role::Server,
        None,
    );
    assert_eq!(server.read().unwrap().into_data().as_ref(), payload);
    assert!(server.read().is_err(), "exactly one complete frame was written");
    let Some(StreamingWorkerEvent::OutgoingLog(message)) = command.completion() else {
        panic!("completed log");
    };
    assert!(message.len() < 8192 && message.contains("100000 bytes"));
}

#[test]
fn invalid_text_and_oversized_payloads_are_rejected_before_any_protocol_write() {
    assert!(Outbound::new(StreamingSendFrameKind::Binary, &vec![0; COMMAND_BYTES], 1, false).is_err());
    let mut command = Outbound::new(StreamingSendFrameKind::Text, &[255], 1, false).unwrap();
    assert!(command.prepare().unwrap_err().contains("UTF-8"));
}

#[test]
fn an_absolute_deadline_reports_an_unconfirmed_effect_without_restarting_the_frame() {
    let wire = Wire {
        input: Cursor::new(vec![]),
        output: vec![],
        actions: VecDeque::new(),
    };
    let mut socket = WebSocket::from_raw_socket(wire, Role::Client, None);
    let mut command = Outbound::new(StreamingSendFrameKind::Binary, &[7], 2, false).unwrap();
    command.prepare().unwrap();
    command.started = Some(Instant::now() - WRITE_DEADLINE);
    assert!(command.write_turn(&mut socket).is_err());
    assert!(socket.get_ref().output.is_empty());
    assert!(
        matches!(command.failure("deadline"), StreamingWorkerEvent::Error(error) if error.contains("not be replayed"))
    );
}

use super::*;

#[test]
fn canonical_websocket_broadcast_admission_is_shallow_and_encoding_is_native() {
    use crate::app::module::common::streaming::commands::preparation::tests::native_payload;
    let pool = golden_io::PayloadPool::new("ws-server-canonical", 1, 128 * 1024).unwrap();
    let payload = native_payload(&pool, vec![9; 100_000]);
    let pointer = payload.request.bytes.as_ptr() as usize;
    let command = Command::new(StreamingSendFrameKind::Binary, SendInput::Prepared(&payload), false).unwrap();
    assert_eq!(command.bytes.as_ptr() as usize, pointer);
    drop(payload);
    std::thread::spawn(move || {
        let broadcast = Broadcast::new(command, [1].into_iter()).unwrap();
        let Message::Binary(bytes) = broadcast.message else {
            panic!("native binary broadcast")
        };
        assert_ne!(bytes.as_ptr() as usize, pointer);
        assert_eq!(bytes.as_ref(), vec![9; 100_000]);
    })
    .join()
    .unwrap();
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}
use std::{
    collections::VecDeque,
    io::{self, Cursor},
};
use tokio_tungstenite::tungstenite::protocol::Role;

struct Wire {
    input: Cursor<Vec<u8>>,
    output: Vec<u8>,
    writes: VecDeque<io::Result<usize>>,
}
impl Read for Wire {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.input.read(bytes)
    }
}
impl Write for Wire {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let size = self.writes.pop_front().unwrap_or(Ok(bytes.len()))?.min(bytes.len());
        self.output.extend_from_slice(&bytes[..size]);
        Ok(size)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
fn wire(writes: VecDeque<io::Result<usize>>) -> WebSocket<Wire> {
    WebSocket::from_raw_socket(
        Wire {
            input: Cursor::new(vec![]),
            output: vec![],
            writes,
        },
        Role::Server,
        Some(crate::app::module::common::streaming::websocket::websocket_config()),
    )
}

#[test]
fn partial_writes_flush_once_per_original_recipient_without_reencoding_the_message() {
    let payload = vec![7; 200_000];
    let mut broadcast = Broadcast::new(
        Command::new(StreamingSendFrameKind::Binary, &payload, false).unwrap(),
        [10, 20].into_iter(),
    )
    .unwrap();
    let mut first = wire(VecDeque::from([
        Ok(3),
        Err(io::ErrorKind::WouldBlock.into()),
        Err(io::ErrorKind::WouldBlock.into()),
        Ok(5),
    ]));
    let mut second = wire(VecDeque::new());
    assert_eq!(broadcast.next_recipient(), Some(0));
    broadcast.write_turn(0, &mut first).unwrap();
    assert_eq!(broadcast.next_recipient(), Some(1));
    broadcast.write_turn(1, &mut second).unwrap();
    broadcast.write_turn(0, &mut first).unwrap();
    broadcast.write_turn(0, &mut first).unwrap();
    assert_eq!(broadcast.next_recipient(), None);
    for socket in [&first, &second] {
        let mut reader = WebSocket::from_raw_socket(
            Wire {
                input: Cursor::new(socket.get_ref().output.clone()),
                output: vec![],
                writes: VecDeque::new(),
            },
            Role::Client,
            None,
        );
        assert_eq!(reader.read().unwrap().into_data().as_ref(), payload);
        assert!(reader.read().is_err(), "exactly one frame per peer");
    }
}

#[test]
fn recipient_identity_and_deadline_do_not_restart_or_retarget_a_partial_frame() {
    let mut broadcast = Broadcast::new(
        Command::new(StreamingSendFrameKind::Text, b"message", false).unwrap(),
        [9, 15].into_iter(),
    )
    .unwrap();
    let mut socket = wire(VecDeque::from([Ok(1), Err(io::ErrorKind::WouldBlock.into())]));
    broadcast.write_turn(0, &mut socket).unwrap();
    let prefix = socket.get_ref().output.clone();
    broadcast.recipients[0].started = Some(Instant::now() - Duration::from_secs(5));
    assert!(broadcast.write_turn(0, &mut socket).unwrap_err().contains("no replay"));
    assert_eq!(socket.get_ref().output, prefix);
    broadcast.fail(0);
    assert_eq!(broadcast.next_recipient(), Some(1));
    assert_eq!(broadcast.id(1), 15);
}

#[test]
fn invalid_utf8_and_retained_command_size_are_rejected_before_protocol_writes() {
    assert!(Command::new(StreamingSendFrameKind::Binary, vec![0; COMMAND_BYTES], false).is_err());
    assert!(Broadcast::new(
        Command::new(StreamingSendFrameKind::Text, &[255], false).unwrap(),
        [1].into_iter()
    )
    .is_err());
}

#[test]
fn completion_waits_for_flush_counts_confirmed_peers_and_keeps_a_bounded_prefix() {
    let mut payload = vec![b'a'; 9000];
    payload[8996..].copy_from_slice(b"TAIL");
    let mut broadcast = Broadcast::new(
        Command::new(StreamingSendFrameKind::Text, &payload, true).unwrap(),
        [1, 2].into_iter(),
    )
    .unwrap();
    let mut socket = wire(VecDeque::from([Ok(1), Err(io::ErrorKind::WouldBlock.into())]));
    broadcast.write_turn(0, &mut socket).unwrap();
    assert!(
        broadcast.recipients[0].state == RecipientState::Pending,
        "submitted is not flushed"
    );
    broadcast.write_turn(0, &mut socket).unwrap();
    broadcast.fail(1);
    let Some(super::super::Event::OutgoingLog(message)) = broadcast.completion() else {
        panic!("confirmed subset")
    };
    assert!(message.contains("9000 bytes") && message.contains("1/2 original client(s)"));
    assert!(!message.contains("TAIL") && message.len() < 4096);
    for log in [false, true] {
        let mut broadcast = Broadcast::new(
            Command::new(StreamingSendFrameKind::Text, b"failed", log).unwrap(),
            [1].into_iter(),
        )
        .unwrap();
        broadcast.fail(0);
        assert!(broadcast.completion().is_none());
    }
    let mut silent = Broadcast::new(
        Command::new(StreamingSendFrameKind::Binary, b"silent", false).unwrap(),
        [1].into_iter(),
    )
    .unwrap();
    silent.write_turn(0, &mut wire(VecDeque::new())).unwrap();
    assert!(silent.completion().is_none());
}

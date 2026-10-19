use super::*;

#[test]
fn canonical_serial_cursor_keeps_the_original_allocation_through_partial_writes() {
    use crate::app::module::common::streaming::commands::preparation::tests::native_payload;
    let pool = golden_io::PayloadPool::new("serial-canonical", 1, 128 * 1024).unwrap();
    let payload = native_payload(&pool, vec![9; 100_000]);
    let pointer = payload.request.bytes.as_ptr();
    let mut command = Outbound::new(SendInput::Prepared(&payload), 1, false).unwrap();
    assert_eq!(command.bytes.as_ptr(), pointer);
    drop(payload);
    command.prepare().unwrap();
    let mut port = wire([Ok(3), Err(io::ErrorKind::WouldBlock.into())]);
    while !command.write_turn(&mut port).unwrap() {}
    assert_eq!(command.bytes.as_ptr(), pointer);
    assert_eq!(port.bytes, vec![9; 100_000]);
    drop(command);
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}
use std::collections::VecDeque;

struct Wire {
    actions: VecDeque<io::Result<usize>>,
    bytes: Vec<u8>,
    calls: usize,
    largest_slice: usize,
}
impl Write for Wire {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.calls += 1;
        self.largest_slice = self.largest_slice.max(bytes.len());
        let written = self.actions.pop_front().unwrap_or(Ok(bytes.len()))?.min(bytes.len());
        self.bytes.extend_from_slice(&bytes[..written]);
        Ok(written)
    }
    fn flush(&mut self) -> io::Result<()> {
        panic!("serial ownership must not invoke an unbounded driver flush")
    }
}
fn wire(actions: impl IntoIterator<Item = io::Result<usize>>) -> Wire {
    Wire {
        actions: actions.into_iter().collect(),
        bytes: Vec::new(),
        calls: 0,
        largest_slice: 0,
    }
}

#[test]
fn partial_writes_and_retryable_errors_preserve_only_the_original_suffix_in_bounded_slices() {
    let payload = vec![7; 100_000];
    let mut command = Outbound::new(&payload, 9, false).unwrap();
    let mut port = wire([
        Ok(3),
        Err(io::ErrorKind::WouldBlock.into()),
        Err(io::ErrorKind::Interrupted.into()),
        Ok(5),
    ]);
    assert!(!command.write_turn(&mut port).unwrap());
    assert_eq!(command.offset, 3);
    assert!(!command.write_turn(&mut port).unwrap());
    assert!(!command.write_turn(&mut port).unwrap());
    while !command.write_turn(&mut port).unwrap() {}
    assert_eq!(port.bytes, payload);
    assert!(port.largest_slice <= WRITE_BYTES);
    assert_eq!(command.generation, 9);
}

#[test]
fn a_timeout_is_an_unconfirmed_effect_and_never_restarts_the_original_frame() {
    let mut command = Outbound::new(b"payload", 1, false).unwrap();
    let mut port = wire([Ok(2), Err(io::ErrorKind::TimedOut.into())]);
    assert!(!command.write_turn(&mut port).unwrap());
    assert!(command.write_turn(&mut port).is_err());
    assert_eq!(port.bytes, b"pa");
    assert!(
        matches!(command.failure("timeout"), Event::Warning(message) if message.contains("2 bytes confirmed") && message.contains("will not be replayed"))
    );
}

#[test]
fn absolute_deadline_and_zero_progress_do_not_resubmit_bytes() {
    let mut command = Outbound::new(b"payload", 1, false).unwrap();
    let mut port = wire([Ok(2)]);
    assert!(!command.write_turn(&mut port).unwrap());
    command.started = Some(Instant::now() - WRITE_DEADLINE);
    assert!(command.write_turn(&mut port).unwrap_err().contains("deadline"));
    assert_eq!(port.calls, 1);
    let mut command = Outbound::new(b"payload", 1, false).unwrap();
    assert!(command.write_turn(&mut wire([Ok(0)])).is_err());
}

#[test]
fn oversized_admission_is_rejected_before_a_driver_can_observe_it() {
    assert!(Outbound::new(vec![0; COMMAND_BYTES], 1, false)
        .err()
        .unwrap()
        .contains("256 KiB"));
}

#[test]
fn completion_logs_only_confirmed_output_and_caps_the_retained_byte_prefix() {
    let mut payload = vec![b'a'; 9000];
    payload[8996..].copy_from_slice(b"TAIL");
    let mut command = Outbound::new(&payload, 1, true).unwrap();
    let mut port = wire([Ok(2), Err(io::ErrorKind::WouldBlock.into())]);
    while !command.write_turn(&mut port).unwrap() {}
    let Some(Event::OutgoingLog(message)) = command.completion() else {
        panic!("native completion")
    };
    assert!(message.contains("9000 bytes") && !message.contains("TAIL"));
    assert!(message.len() < 4096);
    let mut silent = Outbound::new(b"silent", 1, false).unwrap();
    assert!(silent.write_turn(&mut wire([])).unwrap());
    assert!(silent.completion().is_none());
}

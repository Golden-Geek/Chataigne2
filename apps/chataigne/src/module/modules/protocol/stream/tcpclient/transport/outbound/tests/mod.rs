use super::*;

#[test]
fn canonical_tcp_cursor_keeps_the_original_allocation_through_partial_writes() {
    use crate::app::module::common::streaming::commands::preparation::tests::native_payload;
    let pool = golden_io::PayloadPool::new("tcp-canonical", 1, 128 * 1024).unwrap();
    let payload = native_payload(&pool, vec![9; 100_000]);
    let pointer = payload.request.bytes.as_ptr();
    let mut command = Outbound::new(SendInput::Prepared(&payload), 1, false).unwrap();
    assert_eq!(command.bytes.as_ptr(), pointer);
    drop(payload);
    command.prepare().unwrap();
    let mut writer = Writer {
        actions: VecDeque::from([Ok(3), Err(io::ErrorKind::WouldBlock.into())]),
        bytes: Vec::new(),
        largest: 0,
    };
    while command.write_turn(&mut writer).unwrap() != WriteTurn::Complete {}
    assert_eq!(command.bytes.as_ptr(), pointer);
    assert_eq!(writer.bytes, vec![9; 100_000]);
    drop(command);
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}
use std::collections::VecDeque;

struct Writer {
    actions: VecDeque<io::Result<usize>>,
    bytes: Vec<u8>,
    largest: usize,
}
impl Write for Writer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.largest = self.largest.max(bytes.len());
        let count = self.actions.pop_front().unwrap_or(Ok(bytes.len()))?;
        let count = count.min(bytes.len());
        self.bytes.extend_from_slice(&bytes[..count]);
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn partial_writes_and_would_block_resume_the_suffix_once_with_a_bounded_slice() {
    let bytes: Vec<_> = (0..200_000).map(|index| index as u8).collect();
    let mut command = Outbound::new(bytes.clone(), 1, false).unwrap();
    let mut writer = Writer {
        actions: VecDeque::from([
            Ok(7),
            Err(io::ErrorKind::WouldBlock.into()),
            Err(io::ErrorKind::Interrupted.into()),
            Ok(13),
        ]),
        bytes: Vec::new(),
        largest: 0,
    };
    assert_eq!(
        command.write_turn(&mut writer).unwrap(),
        WriteTurn::Pending { progressed: true }
    );
    assert_eq!(command.offset, 7);
    for _ in 0..2 {
        assert_eq!(
            command.write_turn(&mut writer).unwrap(),
            WriteTurn::Pending { progressed: false }
        );
        assert_eq!(command.offset, 7);
    }
    while command.write_turn(&mut writer).unwrap() != WriteTurn::Complete {}
    assert_eq!(writer.bytes, bytes);
    assert!(writer.largest <= WRITE_BYTES_PER_TURN);
    assert!(command.completion().is_none());
}

#[test]
fn failure_and_absolute_deadline_preserve_written_count_without_restarting_a_command() {
    let mut command = Outbound::new(vec![7; 20], 1, true).unwrap();
    let mut writer = Writer {
        actions: VecDeque::from([Ok(5), Err(io::ErrorKind::BrokenPipe.into())]),
        bytes: Vec::new(),
        largest: 0,
    };
    command.write_turn(&mut writer).unwrap();
    assert_eq!(
        command.write_turn(&mut writer).unwrap_err().kind(),
        io::ErrorKind::BrokenPipe
    );
    assert!(
        matches!(command.failure("closed"), StreamingWorkerEvent::Error(error) if error.contains("5/20") && error.contains("not be replayed"))
    );
    command.started = Some(Instant::now() - WRITE_DEADLINE);
    assert_eq!(
        command.write_turn(&mut writer).unwrap_err().kind(),
        io::ErrorKind::TimedOut
    );
    assert_eq!(writer.bytes, [7; 5]);
}

#[test]
fn retained_capacity_is_rejected_before_admission_and_logs_use_a_bounded_prefix() {
    let bytes = Vec::with_capacity(COMMAND_BYTES);
    assert!(Outbound::new(bytes, 1, false).is_err());
    let mut command = Outbound::new(vec![255; 100_000], 1, true).unwrap();
    let mut writer = Writer {
        actions: VecDeque::new(),
        bytes: Vec::new(),
        largest: 0,
    };
    while command.write_turn(&mut writer).unwrap() != WriteTurn::Complete {}
    let Some(StreamingWorkerEvent::OutgoingLog(message)) = command.completion() else {
        panic!("expected completed send log")
    };
    assert!(message.contains("100000 bytes"));
    assert!(message.len() < 8192);
}

use super::*;
use std::collections::VecDeque;

struct Writer {
    actions: VecDeque<io::Result<usize>>,
    bytes: Vec<u8>,
    largest: usize,
}
impl Write for Writer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.largest = self.largest.max(bytes.len());
        let count = self.actions.pop_front().unwrap_or(Ok(bytes.len()))?.min(bytes.len());
        self.bytes.extend_from_slice(&bytes[..count]);
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
fn writer(actions: Vec<io::Result<usize>>) -> Writer {
    Writer {
        actions: actions.into(),
        bytes: Vec::new(),
        largest: 0,
    }
}

#[test]
fn partial_writes_and_would_block_preserve_each_client_suffix_without_replay() {
    let bytes: Vec<_> = (0..200_000).map(|index| index as u8).collect();
    let mut command = Broadcast::new(bytes.clone(), [1, 2].into_iter());
    let mut writers = [
        writer(vec![
            Ok(7),
            Err(io::ErrorKind::WouldBlock.into()),
            Err(io::ErrorKind::Interrupted.into()),
            Ok(13),
        ]),
        writer(vec![]),
    ];
    let mut turns = 0;
    while let Some(index) = command.next_recipient() {
        command.write_turn(index, &mut writers[index]).unwrap();
        turns += 1;
        assert!(turns < 30);
    }
    for writer in writers {
        assert_eq!(writer.bytes, bytes);
        assert!(writer.largest <= WRITE_BYTES);
    }
}

#[test]
fn a_blocked_client_does_not_hold_up_healthy_clients_and_has_an_absolute_deadline() {
    let bytes = vec![7; 100_000];
    let mut command = Broadcast::new(bytes.clone(), [11, 22].into_iter());
    let mut blocked = writer((0..8).map(|_| Err(io::ErrorKind::WouldBlock.into())).collect());
    let mut healthy = writer(vec![]);
    for _ in 0..4 {
        let index = command.next_recipient().unwrap();
        command
            .write_turn(index, if index == 0 { &mut blocked } else { &mut healthy })
            .unwrap();
    }
    assert_eq!(healthy.bytes, bytes);
    assert!(blocked.bytes.is_empty());
    assert_eq!(command.next_recipient(), Some(0));
    command.started = Instant::now() - WRITE_DEADLINE;
    assert_eq!(
        command.write_turn(0, &mut blocked).unwrap_err().kind(),
        io::ErrorKind::TimedOut
    );
    assert!(command.failure(0, "deadline").contains("0/100000"));
    command.finish(0);
    assert!(command.next_recipient().is_none());
}

#[test]
fn original_recipient_identity_and_written_prefix_survive_failure() {
    let mut command = Broadcast::new(vec![9; 20], [71].into_iter());
    let mut writer = writer(vec![Ok(5), Err(io::ErrorKind::BrokenPipe.into())]);
    command.write_turn(0, &mut writer).unwrap();
    assert_eq!(
        command.write_turn(0, &mut writer).unwrap_err().kind(),
        io::ErrorKind::BrokenPipe
    );
    assert_eq!(command.id(0), 71);
    let message = command.failure(0, "closed");
    assert!(message.contains("5/20") && message.contains("not be replayed"));
    assert_eq!(writer.bytes, vec![9; 5]);
    command.finish(0);
    assert_eq!(command.next_recipient(), None);
}

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
    let mut command = Broadcast::new(bytes.clone(), [1, 2].into_iter(), false);
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
    let mut command = Broadcast::new(bytes.clone(), [11, 22].into_iter(), false);
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
    command.fail(0);
    assert!(command.next_recipient().is_none());
}

#[test]
fn original_recipient_identity_and_written_prefix_survive_failure() {
    let mut command = Broadcast::new(vec![9; 20], [71].into_iter(), false);
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
    command.fail(0);
    assert_eq!(command.next_recipient(), None);
}

#[test]
fn broadcast_completion_counts_only_confirmed_original_recipients_and_bounds_feedback() {
    let mut bytes = vec![b'a'; 9000];
    bytes[8996..].copy_from_slice(b"TAIL");
    let mut command = Broadcast::new(bytes, [1, 2, 3].into_iter(), true);
    command.write_turn(0, &mut writer(vec![])).unwrap();
    command.fail(1);
    let mut partial = writer(vec![Ok(2), Err(io::ErrorKind::BrokenPipe.into())]);
    command.write_turn(2, &mut partial).unwrap();
    assert!(command.write_turn(2, &mut partial).is_err());
    command.fail(2);
    assert!(command.next_recipient().is_none());
    let Some(super::super::TcpServerWorkerEvent::OutgoingLog(message)) = command.completion() else {
        panic!("confirmed subset")
    };
    assert!(message.contains("9000 bytes") && message.contains("1/3 original client(s)"));
    assert!(!message.contains("TAIL") && message.len() < 4096);
    let mut failed = Broadcast::new(b"failed".to_vec(), [1].into_iter(), true);
    failed.fail(0);
    assert!(failed.completion().is_none());
    let mut silent = Broadcast::new(b"silent".to_vec(), [1].into_iter(), false);
    silent.write_turn(0, &mut writer(vec![])).unwrap();
    assert!(silent.completion().is_none());
}

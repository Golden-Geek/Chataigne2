use super::*;

#[test]
fn guards_fence_advances_other_lifetimes_and_owner_retirement() {
    let source = RevisionSource::default();
    let reader = source.reader();
    let zero = reader.guard_at(0);
    assert!(zero.is_current());
    assert_eq!(source.advance(), 1);
    assert!(!zero.is_current());
    assert!(!reader.guard_at(0).is_current());
    let current = reader.guard_at(1);
    let other = RevisionSource::default();
    other.advance();
    drop(other);
    assert!(current.is_current());
    drop(source);
    assert!(!current.is_current());
    assert!(!reader.guard_at(reader.current()).is_current());
}

#[test]
fn cross_thread_advance_invalidates_existing_guard() {
    let source = RevisionSource::default();
    let guard = source.reader().guard_at(0);
    let (send, receive) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || {
        source.advance();
        send.send(source).unwrap();
    });
    let source = receive.recv().unwrap();
    assert!(!guard.is_current());
    assert!(source.reader().guard_at(1).is_current());
    worker.join().unwrap();
}

#[test]
fn exhausted_revision_cannot_wrap_and_revive_an_old_guard() {
    let source = RevisionSource::default();
    let old = source.reader().guard_at(0);
    source.0.revision.store(u64::MAX, Ordering::Release);
    assert!(std::panic::catch_unwind(|| source.advance()).is_err());
    assert_eq!(source.reader().current(), u64::MAX);
    assert!(!old.is_current());
}

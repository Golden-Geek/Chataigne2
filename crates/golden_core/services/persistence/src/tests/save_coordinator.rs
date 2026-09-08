use std::fs;
use std::io;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Barrier, Mutex, mpsc};
use std::thread;
use std::time::Duration;

use tempfile::tempdir;

use crate::file_store::{
    FileTransactionStage, restore_primary_from_backup_observing, write_file_atomically_with_recovery_observing,
};
use crate::{
    PersistenceCoordinator, PersistenceCoordinatorError, RecoveryPaths, normalize_destination_identity,
    read_recovery_candidates, write_file_atomically_with_recovery,
};

#[test]
fn same_destination_commits_follow_acceptance_order_when_encoding_finishes_reversed() {
    let directory = tempdir().expect("temporary persistence directory");
    let target = directory.path().join("ordered.noisette");
    let coordinator = PersistenceCoordinator::new(1, 4);
    let first = coordinator.accept_save(&target, 1, 10).expect("first ticket");
    let second = coordinator.accept_save(&target, 1, 11).expect("second ticket");
    let (second_started_tx, second_started_rx) = mpsc::channel();
    let second_coordinator = coordinator.clone();
    let second_thread = thread::spawn(move || {
        second_started_tx.send(()).expect("second start signal");
        second_coordinator.commit_save(second, b"second", |_| ())
    });
    second_started_rx.recv().expect("second commit attempted");

    let first_result = coordinator
        .commit_save(first, b"first", |_| ())
        .expect("first accepted save commits first");
    assert_eq!(fs::read(&target).expect("first primary"), b"first");
    assert_eq!(first_result.ticket.request_id, 1);

    let second_result = second_thread
        .join()
        .expect("second commit thread")
        .expect("second accepted save commits after first");
    assert_eq!(fs::read(&target).expect("second primary"), b"second");
    assert_eq!(second_result.ticket.request_id, 2);
}

#[test]
fn lexical_and_canonical_aliases_share_one_destination_identity() {
    let directory = tempdir().expect("temporary persistence directory");
    let nested = directory.path().join("nested");
    fs::create_dir(&nested).expect("nested directory");
    let lexical_alias = nested.join("..").join("alias.noisette");
    let direct = directory.path().join("alias.noisette");

    assert_eq!(
        normalize_destination_identity(&lexical_alias).expect("lexical identity"),
        normalize_destination_identity(&direct).expect("direct identity")
    );
}

#[test]
fn different_destinations_use_bounded_concurrency() {
    let directory = tempdir().expect("temporary persistence directory");
    let active = Arc::new(AtomicUsize::new(0));
    let peak = Arc::new(AtomicUsize::new(0));
    let entered = Arc::new(Barrier::new(3));
    let release = Arc::new(Barrier::new(3));
    let coordinator = PersistenceCoordinator::with_writer_for_tests(1, 2, {
        let active = Arc::clone(&active);
        let peak = Arc::clone(&peak);
        let entered = Arc::clone(&entered);
        let release = Arc::clone(&release);
        move |path, contents| {
            let now = active.fetch_add(1, Ordering::SeqCst) + 1;
            peak.fetch_max(now, Ordering::SeqCst);
            entered.wait();
            release.wait();
            let result = write_file_atomically_with_recovery(path, contents);
            active.fetch_sub(1, Ordering::SeqCst);
            result
        }
    });

    let mut workers = Vec::new();
    for index in 0..2 {
        let target = directory.path().join(format!("parallel-{index}.noisette"));
        let ticket = coordinator.accept_save(&target, 1, index).expect("parallel ticket");
        let coordinator = coordinator.clone();
        workers.push(thread::spawn(move || {
            coordinator.commit_save(ticket, format!("value-{index}").as_bytes(), |_| ())
        }));
    }

    entered.wait();
    assert_eq!(peak.load(Ordering::SeqCst), 2);
    release.wait();
    for worker in workers {
        worker.join().expect("commit worker").expect("parallel commit");
    }
}

#[test]
fn replacement_waits_for_active_commit_and_invalidates_unstarted_old_generation() {
    let directory = tempdir().expect("temporary persistence directory");
    let active_target = directory.path().join("active.noisette");
    let pending_target = directory.path().join("pending.noisette");
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let release_rx = Arc::new(Mutex::new(release_rx));
    let coordinator = PersistenceCoordinator::with_writer_for_tests(1, 2, {
        let release_rx = Arc::clone(&release_rx);
        move |path, contents| {
            entered_tx.send(()).expect("writer entry signal");
            release_rx
                .lock()
                .expect("writer release lock")
                .recv()
                .expect("writer release");
            write_file_atomically_with_recovery(path, contents)
        }
    });
    let active_ticket = coordinator.accept_save(&active_target, 1, 4).expect("active ticket");
    let pending_ticket = coordinator.accept_save(&pending_target, 1, 5).expect("pending ticket");

    let active_coordinator = coordinator.clone();
    let active_thread = thread::spawn(move || active_coordinator.commit_save(active_ticket, b"old", |_| ()));
    entered_rx.recv().expect("active transaction entered writer");

    let replacement_coordinator = coordinator.clone();
    let (fence_tx, fence_rx) = mpsc::channel();
    let replacement_thread = thread::spawn(move || {
        let fence = replacement_coordinator
            .begin_generation_replacement(1)
            .expect("replacement fence");
        fence_tx.send(()).expect("fence acquired signal");
        fence.commit(2).expect("replacement generation commit");
    });
    assert!(
        fence_rx.recv_timeout(Duration::from_millis(50)).is_err(),
        "replacement must wait for the complete active transaction"
    );
    release_tx.send(()).expect("release active transaction");
    active_thread
        .join()
        .expect("active commit thread")
        .expect("active commit");
    fence_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("replacement should acquire fence after active save");
    replacement_thread.join().expect("replacement thread");

    let stale = coordinator
        .commit_save(pending_ticket, b"stale", |_| ())
        .expect_err("unstarted old-generation save must not commit");
    assert!(matches!(
        stale,
        PersistenceCoordinatorError::StaleGeneration {
            requested: 1,
            current: 2
        }
    ));
    assert!(!pending_target.exists());
}

#[test]
fn aborted_replacement_releases_pending_saves_without_changing_generation() {
    let directory = tempdir().expect("temporary persistence directory");
    let target = directory.path().join("after-abort.noisette");
    let coordinator = PersistenceCoordinator::new(1, 1);
    let ticket = coordinator.accept_save(&target, 1, 2).expect("pending ticket");
    drop(coordinator.begin_generation_replacement(1).expect("replacement fence"));

    coordinator
        .commit_save(ticket, b"preserved", |_| ())
        .expect("save remains valid after aborted replacement");
    assert_eq!(fs::read(target).expect("saved contents"), b"preserved");
}

#[test]
fn dropped_or_failed_earlier_save_releases_the_destination_queue() {
    let directory = tempdir().expect("temporary persistence directory");
    let target = directory.path().join("retry.noisette");
    let writes = Arc::new(AtomicUsize::new(0));
    let coordinator = PersistenceCoordinator::with_writer_for_tests(1, 1, {
        let writes = Arc::clone(&writes);
        move |path, contents| {
            if writes.fetch_add(1, Ordering::SeqCst) == 0 {
                return Err(io::Error::other("injected first write failure"));
            }
            write_file_atomically_with_recovery(path, contents)
        }
    });
    let canceled = coordinator.accept_save(&target, 1, 1).expect("canceled ticket");
    let failed = coordinator.accept_save(&target, 1, 2).expect("failed ticket");
    let winner = coordinator.accept_save(&target, 1, 3).expect("winning ticket");
    drop(canceled);

    let error = coordinator
        .commit_save(failed, b"failed", |_| ())
        .expect_err("injected write failure");
    assert!(error.to_string().contains("injected first write failure"));
    coordinator
        .commit_save(winner, b"winner", |_| ())
        .expect("later ticket should proceed");
    assert_eq!(fs::read(target).expect("winning primary"), b"winner");
}

#[test]
fn interrupted_write_stages_recover_a_complete_revision_and_allow_a_later_save() {
    let stages = [
        FileTransactionStage::BackupTempReady,
        FileTransactionStage::BackupCommitted,
        FileTransactionStage::JournalTempReady,
        FileTransactionStage::JournalCommitted,
        FileTransactionStage::TargetTempReady,
        FileTransactionStage::TargetCommitted,
        FileTransactionStage::BeforeJournalCleanup,
        FileTransactionStage::JournalCleared,
    ];
    for stage in stages {
        let directory = tempdir().expect("temporary persistence directory");
        let target = directory.path().join("fault.noisette");
        write_file_atomically_with_recovery(&target, b"previous").expect("initial save");
        let error = write_file_atomically_with_recovery_observing(&target, b"next", &mut |current| {
            if current == stage {
                Err(io::Error::other(format!("injected {stage:?}")))
            } else {
                Ok(())
            }
        })
        .expect_err("stage fault should surface");
        assert!(error.to_string().contains("injected"));

        let primary = fs::read(&target).expect("a complete primary remains");
        assert!(primary == b"previous" || primary == b"next");
        let candidates = read_recovery_candidates(&target).expect("recovery candidates remain readable");
        assert!(candidates.primary.is_some() || candidates.backup.is_some());
        write_file_atomically_with_recovery(&target, b"later").expect("later save must recover progress");
        assert_eq!(fs::read(&target).expect("later primary"), b"later");
    }
}

#[test]
fn interrupted_backup_restore_keeps_valid_data_and_can_be_retried() {
    let stages = [
        FileTransactionStage::JournalTempReady,
        FileTransactionStage::JournalCommitted,
        FileTransactionStage::TargetTempReady,
        FileTransactionStage::TargetCommitted,
        FileTransactionStage::BeforeJournalCleanup,
        FileTransactionStage::JournalCleared,
    ];
    for stage in stages {
        let directory = tempdir().expect("temporary persistence directory");
        let target = directory.path().join("restore.noisette");
        write_file_atomically_with_recovery(&target, b"backup-source").expect("initial save");
        write_file_atomically_with_recovery(&target, b"corrupt-next").expect("replacement save");
        let paths = RecoveryPaths::for_target(&target).expect("recovery paths");
        let backup = fs::read(&paths.backup).expect("backup contents");

        let _ = restore_primary_from_backup_observing(&paths, &backup, &mut |current| {
            if current == stage {
                Err(io::Error::other(format!("injected {stage:?}")))
            } else {
                Ok(())
            }
        });
        let primary = fs::read(&target).expect("complete primary after interrupted restore");
        assert!(primary == b"corrupt-next" || primary == backup);
        crate::restore_primary_from_backup(&paths, &backup).expect("restore retry");
        assert_eq!(fs::read(&target).expect("restored primary"), backup);
    }
}

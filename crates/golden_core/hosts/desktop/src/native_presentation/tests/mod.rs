use super::*;
use std::io;
use std::sync::mpsc::{self, TryRecvError};
use std::thread::{self, ThreadId};
use std::time::Duration;

const DEADLINE: Duration = Duration::from_secs(5);

#[test]
fn repeated_stop_requests_wake_a_waiting_worker() {
    let (started_tx, started_rx) = mpsc::channel();
    let (finished_tx, finished_rx) = mpsc::channel();
    let worker = PresentationWorker::spawn(
        NativePresentationTask::new(move |stop| {
            started_tx.send(()).unwrap();
            assert!(stop.wait_timeout(DEADLINE));
            assert!(stop.is_requested());
            assert!(stop.clone().wait_timeout(Duration::ZERO));
            Ok(())
        }),
        move |success| finished_tx.send(success).unwrap(),
    )
    .unwrap();
    started_rx.recv_timeout(DEADLINE).unwrap();
    worker.request_stop();
    worker.request_stop();
    assert!(finished_rx.recv_timeout(DEADLINE).unwrap());
    worker.join().unwrap();
    assert_eq!(finished_rx.try_recv(), Err(TryRecvError::Disconnected));
}

#[test]
fn spontaneous_completion_does_not_require_cancellation() {
    let (finished_tx, finished_rx) = mpsc::channel();
    let worker = PresentationWorker::spawn(
        NativePresentationTask::new(|stop| {
            assert!(!stop.is_requested());
            assert!(!stop.wait_timeout(Duration::ZERO));
            Ok(())
        }),
        move |success| finished_tx.send(success).unwrap(),
    )
    .unwrap();
    assert!(finished_rx.recv_timeout(DEADLINE).unwrap());
    worker.join().unwrap();
    assert_eq!(finished_rx.try_recv(), Err(TryRecvError::Disconnected));
}

#[test]
fn task_error_is_retained_and_reported_once() {
    let (finished_tx, finished_rx) = mpsc::channel();
    let worker = PresentationWorker::spawn(
        NativePresentationTask::new(|_| Err(io::Error::new(io::ErrorKind::InvalidData, "invalid native target"))),
        move |success| finished_tx.send(success).unwrap(),
    )
    .unwrap();
    let error = worker.join().unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert_eq!(error.to_string(), "invalid native target");
    assert!(!finished_rx.recv_timeout(DEADLINE).unwrap());
    assert_eq!(finished_rx.try_recv(), Err(TryRecvError::Disconnected));
}

#[test]
fn task_panic_becomes_an_error_and_reports_completion_once() {
    let (finished_tx, finished_rx) = mpsc::channel();
    let worker = PresentationWorker::spawn(
        NativePresentationTask::new(|_| panic!("native test panic")),
        move |success| finished_tx.send(success).unwrap(),
    )
    .unwrap();
    let error = worker.join().unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::Other);
    assert!(error.to_string().contains("native test panic"));
    assert!(!finished_rx.recv_timeout(DEADLINE).unwrap());
    assert_eq!(finished_rx.try_recv(), Err(TryRecvError::Disconnected));
}

struct DropReport(mpsc::Sender<ThreadId>);

impl Drop for DropReport {
    fn drop(&mut self) {
        let _ = self.0.send(thread::current().id());
    }
}

#[derive(Clone, Copy)]
enum Outcome {
    Success,
    Error,
    Panic,
}

#[test]
fn task_captures_drop_on_the_worker_before_completion_for_every_outcome() {
    let caller = thread::current().id();
    for outcome in [Outcome::Success, Outcome::Error, Outcome::Panic] {
        let (drop_tx, drop_rx) = mpsc::channel();
        let (thread_tx, thread_rx) = mpsc::channel();
        let (finished_tx, finished_rx) = mpsc::channel();
        let capture = DropReport(drop_tx);
        let worker = PresentationWorker::spawn(
            NativePresentationTask::new(move |_| {
                let _capture = capture;
                thread_tx.send(thread::current().id()).unwrap();
                match outcome {
                    Outcome::Success => Ok(()),
                    Outcome::Error => Err(io::Error::other("failed task")),
                    Outcome::Panic => panic!("panicked task"),
                }
            }),
            move |success| finished_tx.send((success, drop_rx.try_recv())).unwrap(),
        )
        .unwrap();
        let worker_id = thread_rx.recv_timeout(DEADLINE).unwrap();
        let (success, drop_thread) = finished_rx.recv_timeout(DEADLINE).unwrap();
        assert_ne!(worker_id, caller);
        assert_eq!(drop_thread.unwrap(), worker_id);
        assert_eq!(success, matches!(outcome, Outcome::Success));
        assert_eq!(worker.join().is_ok(), success);
    }
}

#[test]
fn dropping_the_worker_requests_stop_without_waiting_for_retirement() {
    let (stop_tx, stop_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let (finished_tx, finished_rx) = mpsc::channel();
    let worker = PresentationWorker::spawn(
        NativePresentationTask::new(move |stop| {
            stop_tx.send(stop).unwrap();
            release_rx.recv().unwrap();
            Ok(())
        }),
        move |success| finished_tx.send(success).unwrap(),
    )
    .unwrap();
    let stop = stop_rx.recv_timeout(DEADLINE).unwrap();
    let (dropped_tx, dropped_rx) = mpsc::channel();
    let drop_thread = thread::spawn(move || {
        drop(worker);
        dropped_tx.send(()).unwrap();
    });
    let dropped = dropped_rx.recv_timeout(DEADLINE);
    let requested = stop.is_requested();
    // Always release the task, even if a blocking Drop regression trips the assertion.
    release_tx.send(()).unwrap();
    assert!(dropped.is_ok(), "dropping worker control must not join its task");
    assert!(requested);
    assert!(finished_rx.recv_timeout(DEADLINE).unwrap());
    drop_thread.join().unwrap();
}

use uuid::Uuid;

use super::*;
use crate::native::{DiagnosticContext, GpuSampleStatus, GpuTrace};
use crate::runtime::{CompletionToken, DeviceGeneration};

fn row(serial: u64) -> DiagnosticEvent {
    DiagnosticEvent::Gpu(GpuTrace {
        token: CompletionToken {
            device: DeviceGeneration::new(Uuid::nil(), 1),
            serial,
        },
        context: DiagnosticContext::Renderer,
        status: GpuSampleStatus::PoolBusy,
        begin: None,
        end: None,
        frequency: None,
    })
}

#[test]
fn trace_pressure_retains_bounded_rows_without_overwriting_and_reports_full_loss() {
    let recorder = Recorder::new(DiagnosticsConfig {
        capacity: NonZeroUsize::new(2).unwrap(),
    });
    recorder.push(row(1));
    recorder.push(row(2));
    recorder.push(row(3));
    let snapshot = recorder.snapshot();
    assert_eq!((snapshot.queued, snapshot.written, snapshot.dropped_full), (2, 2, 1));
    let mut drained = Vec::new();
    assert_eq!(recorder.drain(&mut drained, NonZeroUsize::MIN), 1);
    assert_eq!(recorder.drain(&mut drained, NonZeroUsize::new(8).unwrap()), 1);
    let serials = drained
        .iter()
        .map(|event| match event {
            DiagnosticEvent::Gpu(row) => row.token.serial,
            _ => unreachable!(),
        })
        .collect::<Vec<_>>();
    assert_eq!(serials, [1, 2]);
    let snapshot = recorder.snapshot();
    assert_eq!((snapshot.queued, snapshot.drained), (0, 2));
}

#[test]
fn a_single_slot_can_be_drained_and_reused_without_losing_its_next_row() {
    let recorder = Recorder::new(DiagnosticsConfig {
        capacity: NonZeroUsize::MIN,
    });
    let mut drained = Vec::new();
    for serial in 1..=16 {
        recorder.push(row(serial));
        assert_eq!(recorder.snapshot().queued, 1);
        assert_eq!(recorder.drain(&mut drained, NonZeroUsize::MIN), 1);
    }
    let snapshot = recorder.snapshot();
    assert_eq!(
        (
            snapshot.queued,
            snapshot.written,
            snapshot.drained,
            snapshot.dropped_full
        ),
        (0, 16, 16, 0)
    );
}

#[test]
fn simultaneous_native_producers_and_a_budgeted_reader_keep_every_admitted_row() {
    use std::sync::{Arc, Barrier};
    use std::thread;

    const PRODUCERS: usize = 4;
    const ROWS: usize = 4096;
    let recorder = Recorder::new(DiagnosticsConfig {
        capacity: NonZeroUsize::new(PRODUCERS * ROWS).unwrap(),
    });
    let barrier = Arc::new(Barrier::new(PRODUCERS + 1));
    let finished = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let handles = (0..PRODUCERS)
        .map(|producer| {
            let recorder = recorder.clone();
            let barrier = barrier.clone();
            let finished = finished.clone();
            thread::spawn(move || {
                barrier.wait();
                for index in 0..ROWS {
                    recorder.push(row((producer * ROWS + index + 1) as u64));
                }
                finished.fetch_add(1, Ordering::Release);
            })
        })
        .collect::<Vec<_>>();
    barrier.wait();
    let mut rows = Vec::with_capacity(PRODUCERS * ROWS);
    while finished.load(Ordering::Acquire) != PRODUCERS || !recorder.0.rows.is_empty() {
        recorder.drain(&mut rows, NonZeroUsize::new(17).unwrap());
        thread::yield_now();
    }
    for handle in handles {
        handle.join().unwrap();
    }
    recorder.drain(&mut rows, NonZeroUsize::new(PRODUCERS * ROWS).unwrap());
    let mut previous = [0; PRODUCERS];
    for event in &rows {
        let DiagnosticEvent::Gpu(event) = event else {
            unreachable!()
        };
        let serial = event.token.serial as usize;
        let producer = (serial - 1) / ROWS;
        let index = (serial - 1) % ROWS + 1;
        assert_eq!(index, previous[producer] + 1, "producer FIFO and exact-once delivery");
        previous[producer] = index;
    }
    assert_eq!(previous, [ROWS; PRODUCERS]);
    let snapshot = recorder.snapshot();
    assert_eq!(
        (
            snapshot.queued,
            snapshot.written,
            snapshot.drained,
            snapshot.dropped_full
        ),
        (0, (PRODUCERS * ROWS) as u64, (PRODUCERS * ROWS) as u64, 0)
    );
}

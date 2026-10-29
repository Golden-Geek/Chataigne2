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
fn trace_pressure_retains_bounded_rows_and_reports_full_and_contended_loss() {
    let recorder = Recorder::new(DiagnosticsConfig {
        capacity: NonZeroUsize::new(2).unwrap(),
    });
    recorder.push(row(1));
    recorder.push(row(2));
    recorder.push(row(3));
    {
        let _consumer = recorder.0.rows.lock().unwrap();
        recorder.push(row(4)); // This must return immediately rather than waiting on the consumer.
    }
    let snapshot = recorder.snapshot();
    assert_eq!(
        (
            snapshot.queued,
            snapshot.written,
            snapshot.dropped_full,
            snapshot.dropped_contended
        ),
        (2, 2, 1, 1)
    );
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

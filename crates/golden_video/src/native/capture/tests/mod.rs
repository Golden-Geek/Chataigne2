use super::*;
use std::sync::{Arc, Mutex};

use crate::TextureId;
use crate::native::{DiagnosticsConfig, SourceUseTrace, timing::Recorder};
use crate::runtime::{CompletionToken, DeviceGeneration};

#[derive(Clone, Default)]
struct Buffer(Arc<Mutex<Vec<u8>>>);
impl Write for Buffer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
fn limits() -> DiagnosticCaptureLimits {
    DiagnosticCaptureLimits {
        max_bytes: NonZeroU64::new(4 * 1024 * 1024).unwrap(),
        max_duration: Duration::from_secs(60),
    }
}
fn configured(capacity: usize) -> (D3d11Control, Recorder) {
    let control = D3d11Control::default();
    let recorder = Recorder::new(DiagnosticsConfig {
        capacity: NonZeroUsize::new(capacity).unwrap(),
    });
    *control.diagnostics.lock().unwrap() = Some(recorder.clone());
    (control, recorder)
}
fn event() -> DiagnosticEvent {
    DiagnosticEvent::SourceUse(SourceUseTrace {
        token: CompletionToken {
            device: DeviceGeneration::new(uuid::Uuid::new_v4(), 1),
            serial: 1,
        },
        texture: TextureId::new(1).unwrap(),
        descriptor: crate::TextureDescriptor {
            extent: crate::Extent2D::new(1, 1).unwrap(),
            format: crate::PixelFormat::Rgba8Unorm,
            color_encoding: crate::ColorEncoding::Srgb,
            alpha_mode: crate::AlphaMode::Straight,
        },
        sequence: 0,
        stream: None,
        publication: None,
        software_upload: None,
        frame: None,
        requested_sequence: None,
        requested_position_nanos: None,
    })
}
fn rows(buffer: &Buffer) -> Vec<serde_json::Value> {
    String::from_utf8(buffer.0.lock().unwrap().clone())
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

#[cfg(feature = "ffmpeg")]
#[test]
fn snapshots_include_source_pool_storage_until_the_last_observer_retires() {
    let budget = GpuFrameBudget::new(4096);
    let owner = budget
        .reserve(2048, crate::native::AllocationKind::DecoderPool)
        .unwrap();
    let mut observer = Some(owner.clone());
    drop(owner);
    for retained in [true, false] {
        let buffer = Buffer::default();
        DiagnosticCapture::spawn(D3d11Control::default(), buffer.clone(), limits(), Some(budget.clone()))
            .unwrap()
            .finish()
            .unwrap();
        let snapshot = rows(&buffer)
            .into_iter()
            .rev()
            .find(|row| row["kind"] == "snapshot")
            .unwrap();
        assert_eq!(
            snapshot["source_gpu"]["retained_bytes"],
            if retained { 2048 } else { 0 }
        );
        assert_eq!(
            snapshot["source_gpu"]["decoder_pools"]["allocations"],
            u64::from(retained)
        );
        assert_eq!(snapshot["source_gpu"]["peak_bytes"], 2048);
        if retained {
            // The observer, not a retired decoder/control owner, keeps storage charged.
            drop(observer.take());
        }
    }
}

#[test]
fn final_drain_covers_more_than_one_drain_chunk_and_preserves_wire_events() {
    let (control, recorder) = configured(8192);
    for _ in 0..5000 {
        recorder.push(event());
    }
    let buffer = Buffer::default();
    let summary = DiagnosticCapture::spawn(control, buffer.clone(), limits(), None)
        .unwrap()
        .finish()
        .unwrap();
    assert_eq!(summary.events, 5000);
    assert_eq!(summary.diagnostics.queued, 0);
    assert_eq!(summary.diagnostics.dropped_full, 0);
    assert!(!summary.duration_limit);
    let rows = rows(&buffer);
    assert_eq!(rows.first().unwrap()["kind"], "capture");
    assert_eq!(rows.last().unwrap()["kind"], "capture_end");
    let events: Vec<_> = rows.iter().filter(|row| row["kind"] == "diagnostic").collect();
    assert_eq!(events.len(), 5000);
    assert!(matches!(
        serde_json::from_value::<DiagnosticEvent>(events[0]["event"].clone()).unwrap(),
        DiagnosticEvent::SourceUse(_)
    ));
}

#[test]
fn ring_loss_and_disabled_observations_are_never_success_shaped_counts() {
    let (control, recorder) = configured(2);
    for _ in 0..3 {
        recorder.push(event());
    }
    let summary = DiagnosticCapture::spawn(control, Buffer::default(), limits(), None)
        .unwrap()
        .finish()
        .unwrap();
    assert_eq!(summary.events, 2);
    assert_eq!(summary.diagnostics.dropped_full, 1);
    let summary = DiagnosticCapture::spawn(D3d11Control::default(), Buffer::default(), limits(), None)
        .unwrap()
        .finish()
        .unwrap();
    assert!(!summary.diagnostics.enabled);
    assert_eq!(summary.events, 0);
}

#[test]
fn byte_ceiling_and_broken_writer_propagate_failure_without_a_completion_record() {
    let buffer = Buffer::default();
    let mut bound = limits();
    bound.max_bytes = NonZeroU64::new(64).unwrap();
    let error = DiagnosticCapture::spawn(D3d11Control::default(), buffer.clone(), bound, None)
        .unwrap()
        .finish()
        .unwrap_err();
    assert!(error.to_string().contains("byte limit"));
    assert!(buffer.0.lock().unwrap().len() <= 64);
    assert!(
        !String::from_utf8(buffer.0.lock().unwrap().clone())
            .unwrap()
            .contains("capture_end")
    );
    struct Broken;
    impl Write for Broken {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::Error::other("disk unavailable"))
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let error = DiagnosticCapture::spawn(D3d11Control::default(), Broken, limits(), None)
        .unwrap()
        .finish()
        .unwrap_err();
    assert!(error.to_string().contains("disk unavailable"));
}

#[test]
fn flush_error_is_reported_even_if_a_complete_end_record_was_written() {
    struct FlushFailure(Buffer);
    impl Write for FlushFailure {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.0.write(bytes)
        }
        fn flush(&mut self) -> io::Result<()> {
            Err(io::Error::other("flush failed"))
        }
    }
    let buffer = Buffer::default();
    let error = DiagnosticCapture::spawn(D3d11Control::default(), FlushFailure(buffer.clone()), limits(), None)
        .unwrap()
        .finish()
        .unwrap_err();
    assert!(error.to_string().contains("flush failed"));
    assert_eq!(rows(&buffer).last().unwrap()["kind"], "capture_end");
}

#[test]
fn capture_duration_is_bounded_and_limit_is_explicit() {
    let buffer = Buffer::default();
    let mut bound = limits();
    bound.max_duration = Duration::from_millis(1);
    let capture = DiagnosticCapture::spawn(D3d11Control::default(), buffer.clone(), bound, None).unwrap();
    // Wait for this exact capture's terminal record instead of assuming thread scheduling.
    let deadline = Instant::now() + Duration::from_secs(5);
    while !String::from_utf8(buffer.0.lock().unwrap().clone())
        .unwrap()
        .contains("capture_end")
    {
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(5));
    }
    assert!(capture.finish().unwrap().duration_limit);
    bound.max_duration = Duration::ZERO;
    assert!(DiagnosticCapture::spawn(D3d11Control::default(), Buffer::default(), bound, None).is_err());
}

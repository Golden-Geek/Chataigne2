use super::*;
use crate::image::ImageData;
use crate::native::timing::Recorder;
use crate::native::{DiagnosticContext, DiagnosticEvent, DiagnosticsConfig, GpuSampleStatus, SourceSubmissionSink};
use std::{num::NonZeroUsize, sync::Arc};

#[test]
#[ignore = "explicit Intel source publication/upload identity and retained pixel regression"]
fn hardware_intel_gpu_publication_identity_is_independent_of_upload_version() {
    let adapter = adapters()
        .unwrap()
        .into_iter()
        .find(|adapter| adapter.vendor_id == 0x8086 && !adapter.software)
        .unwrap();
    let desc = descriptor(2, ColorEncoding::Linear, AlphaMode::Straight);
    let fixture = prepared_image(desc, &[0; 8]);
    let config = DiagnosticsConfig {
        capacity: NonZeroUsize::new(1024).unwrap(),
    };
    let mut first = Renderer::with_diagnostics(adapter.id, &fixture, 2, Some(config));
    let mut second = Renderer::with_diagnostics(adapter.id, &fixture, 2, Some(config));
    let budget = GpuFrameBudget::new(24);
    let mut owner = GpuFramePublisher::new_software(desc, adapter.id, &budget, None).unwrap();
    let producer_events = Recorder::new(config);
    let sink = SourceSubmissionSink(producer_events.clone());
    owner.enable_diagnostics(&sink).unwrap();
    assert!(matches!(owner.enable_diagnostics(&sink), Err(GpuFrameError::Layout)));
    let stream = owner.stream();
    // A withdrawal consumes a publication version without performing an upload.
    stream.publish(None).unwrap();
    install(&mut first, stream.clone());
    install(&mut second, stream.clone());
    let image = Arc::new(ImageData::new(desc, vec![127, 127, 127, 255, 127, 127, 127, 255]).unwrap());
    assert!(owner.submit_software(image).unwrap());
    assert!(owner.trace_pending_submission(&sink, None));
    assert!(!owner.trace_pending_submission(&sink, None));
    let image = completed(&mut owner);
    let source_token = *image.submission.as_deref().unwrap();
    let upload = image.software_upload.unwrap();
    assert_eq!(upload.stream, stream.id());
    assert_eq!(upload.sequence, 1);
    stream.publish(Some(image)).unwrap();
    for renderer in [&mut first, &mut second] {
        for _ in 0..2 {
            assert_eq!(renderer.pixel_frame(), vec![[127, 127, 127, 255]; 2]);
        }
        install(renderer, stream.clone());
        assert_eq!(renderer.pixel_frame(), vec![[127, 127, 127, 255]; 2]);
        let mut events = Vec::new();
        renderer
            .backend
            .control
            .drain_diagnostics(&mut events, NonZeroUsize::new(1024).unwrap());
        let uses: Vec<_> = events
            .into_iter()
            .filter_map(|event| match event {
                DiagnosticEvent::SourceUse(row) => Some(row),
                _ => None,
            })
            .collect();
        assert_eq!(uses.len(), 3);
        for row in uses {
            assert_eq!(row.stream, Some(stream.id()));
            assert_eq!(row.sequence, 2, "actual retained publication, not upload version1");
            assert_eq!(row.software_upload.unwrap(), upload);
            let origin = row.publication.unwrap();
            assert_eq!(origin.source_token, source_token);
            assert!(origin.receive_begin_qpc.is_none() && origin.spout_image.is_none());
        }
    }
    stream.publish(None).unwrap();
    for renderer in [&mut first, &mut second] {
        assert!(renderer.pixel_frame().iter().all(|pixel| *pixel == [0; 4]));
        let mut rows = Vec::new();
        renderer
            .backend
            .control
            .drain_diagnostics(&mut rows, NonZeroUsize::new(1024).unwrap());
        assert!(rows.iter().any(|event| matches!(event,
            DiagnosticEvent::SourceUse(row) if row.sequence == 0 && row.publication.is_none())));
    }
    drop(owner);
    let mut rows = Vec::new();
    producer_events.drain(&mut rows, NonZeroUsize::new(1024).unwrap());
    let cpu = rows
        .iter()
        .find_map(|event| match event {
            DiagnosticEvent::SourceCpu(row) => Some(*row),
            _ => None,
        })
        .unwrap();
    let gpu = rows
        .iter()
        .find_map(|event| match event {
            DiagnosticEvent::Gpu(row) => Some(*row),
            _ => None,
        })
        .unwrap();
    assert_eq!(cpu.token, source_token);
    assert_eq!(gpu.token, source_token);
    assert_eq!(cpu.context, DiagnosticContext::FrameProducer { stream: stream.id() });
    assert_eq!(gpu.context, cpu.context);
    assert_eq!(gpu.status, GpuSampleStatus::Valid);
    let wall = cpu.wall.unwrap();
    assert!(wall.begin <= gpu.submitted_qpc.unwrap());
    assert!(gpu.submitted_qpc.unwrap() <= wall.end);
    assert!(wall.end <= gpu.completed_qpc.unwrap());
    assert_eq!(
        rows.iter()
            .filter(|e| matches!(e, DiagnosticEvent::SourceSubmission(_)))
            .count(),
        1
    );
    assert_eq!(producer_events.snapshot().queued, 0);
    assert_eq!(producer_events.snapshot().dropped_full, 0);
    drop((first, second, stream));
    assert_eq!(budget.retained_bytes(), 0);
}

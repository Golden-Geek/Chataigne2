use super::*;
use crate::image::ImageData;
use crate::native::SourceUploadSnapshot;
use crate::native::{DiagnosticsConfig, SourceSubmissionPayload, SourceSubmissionSink, timing::Recorder};
use std::num::NonZeroUsize;
use std::sync::Arc;

#[test]
#[ignore = "explicit Intel native payload submissions, unsampled images and retirement"]
fn hardware_intel_source_upload_totals_include_unsampled_frames_without_counting_busy_calls() {
    let recorder = Recorder::new(DiagnosticsConfig {
        capacity: NonZeroUsize::new(64).unwrap(),
    });
    let sink = SourceSubmissionSink(recorder.clone());
    let adapter = adapters()
        .unwrap()
        .into_iter()
        .find(|a| a.vendor_id == 0x8086 && !a.software)
        .unwrap();
    let budget = GpuFrameBudget::new_with_upload_accounting(24);
    let desc = descriptor(2, ColorEncoding::Srgb, AlphaMode::Straight);
    let image = Arc::new(ImageData::new(desc, vec![127; 8]).unwrap());
    let mut publisher = GpuFramePublisher::new_software(desc, adapter.id, &budget, None).unwrap();
    publisher.record_software_upload_timing();
    let mut held = Vec::new();
    for count in 1..=3 {
        assert!(publisher.submit_software(image.clone()).unwrap());
        assert!(publisher.trace_pending_submission(&sink, None));
        assert!(!publisher.submit_software(image.clone()).unwrap(), "one pending upload");
        assert!(
            !publisher.trace_pending_submission(&sink, None),
            "busy calls cannot retrace the pending upload"
        );
        let image = completed(&mut publisher);
        assert!(
            image.submission.is_none(),
            "CPU upload accounting alone must not enable GPU queries"
        );
        assert!(!publisher.trace_pending_submission(&sink, None));
        let interval = image.software_upload.unwrap().cpu.unwrap();
        assert!(interval.end >= interval.begin);
        held.push(image);
        let snapshot = budget.snapshot().uploads.unwrap();
        assert_eq!(snapshot.software.calls, count);
        assert_eq!(snapshot.software.bytes, count * 8);
        assert_eq!(snapshot.hap.calls, 0);
    }
    let wrong = Arc::new(ImageData::new(descriptor(1, ColorEncoding::Srgb, AlphaMode::Straight), vec![0; 4]).unwrap());
    assert!(matches!(publisher.submit_software(wrong), Err(GpuFrameError::Layout)));
    assert!(
        !publisher.submit_software(image).unwrap(),
        "all three unsampled slots retained"
    );
    let totals = budget.snapshot().uploads.unwrap();
    drop(publisher);
    assert_eq!(
        budget.retained_bytes(),
        24,
        "unsampled images still own their source storage"
    );
    drop(held);
    assert_eq!(budget.retained_bytes(), 0);
    assert_eq!(
        budget.snapshot().uploads,
        Some(totals),
        "terminal counters survive every image"
    );
    let mut replacement = GpuFramePublisher::new_software(desc, adapter.id, &budget, None).unwrap();
    assert!(
        replacement
            .submit_software(Arc::new(ImageData::new(desc, vec![127; 8]).unwrap()))
            .unwrap()
    );
    assert!(replacement.trace_pending_submission(&sink, None));
    drop(completed(&mut replacement));
    drop(replacement);
    assert_eq!(budget.snapshot().uploads.unwrap().software.calls, 4);
    assert_eq!(budget.retained_bytes(), 0);

    for name in ["hap.mov", "hap-alpha.mov", "hap-q.mov"] {
        let mut decoder = Decoder::open(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("src/decode/tests/fixtures")
                .join(name),
            DecodeLimits {
                max_pixels: 4096 * 2160,
                max_cpu_frame_bytes: 64 * 1024 * 1024,
                max_packet_bytes: 16 * 1024 * 1024,
                max_packets_per_frame: 32,
                software_threads: crate::decode::SoftwareDecodeThreads::One,
            },
        )
        .unwrap();
        let mut frame = decoder.next_frame().unwrap();
        let first = frame.as_ref().unwrap().hap_frame().unwrap();
        let admitted = u64::from(first.extent().width()) * u64::from(first.extent().height()) * 4 * 3
            + first.blocks().len() as u64
            + 16;
        let budget = GpuFrameBudget::new_with_upload_accounting(admitted);
        let mut owner = GpuFramePublisher::new_hap(first, adapter.id, &budget, None).unwrap();
        let mut count = 0;
        let mut bytes = 0;
        while let Some(decoded) = frame {
            let hap = decoded.hap_frame().unwrap();
            assert!(owner.submit_hap(hap).unwrap());
            assert!(owner.trace_pending_submission(&sink, None));
            assert!(!owner.submit_hap(hap).unwrap(), "busy HAP submit adds no upload");
            assert!(!owner.trace_pending_submission(&sink, None));
            count += 1;
            bytes += hap.blocks().len() as u64;
            drop(completed(&mut owner));
            frame = decoder.next_frame().unwrap();
        }
        assert_eq!(count, 6);
        drop(owner);
        assert_eq!(budget.retained_bytes(), 0);
        assert_eq!(
            budget.snapshot().peak_bytes,
            admitted,
            "exact unchanged producer storage charge"
        );
        let totals = budget.snapshot().uploads.unwrap();
        assert_eq!(totals.hap.calls, count);
        assert_eq!(totals.hap.bytes, bytes);
        assert!(!totals.hap.overflowed);
        assert_eq!(totals.software.calls, 0);
        eprintln!("{name}: {count} actual compressed uploads, {bytes} payload bytes, retired=0");
    }
    let budget = GpuFrameBudget::new_with_upload_accounting(1024 * 1024);
    let (mut publisher, source) = prepared(adapter.id, D3d11Storage::Nv12, 0, 126, &budget);
    assert!(submit(&mut publisher, &source));
    assert!(publisher.trace_pending_submission(&sink, None));
    drop(completed(&mut publisher));
    drop((publisher, source));
    assert_eq!(budget.retained_bytes(), 0);
    assert_eq!(
        budget.snapshot().uploads,
        Some(SourceUploadSnapshot::default()),
        "resident copies add no CPU payload"
    );
    let mut rows = Vec::new();
    recorder.drain(&mut rows, NonZeroUsize::new(64).unwrap());
    assert_eq!(rows.len(), 23, "four RGBA, eighteen HAP and one resident submission");
    let mut streams = std::collections::HashSet::new();
    let mut submissions = Vec::new();
    let mut software_bytes = 0;
    let mut resident = 0;
    for event in rows {
        let DiagnosticEvent::SourceSubmission(row) = event else {
            panic!("unexpected source event")
        };
        streams.insert(row.stream);
        assert!(!submissions.contains(&(row.stream, row.token)));
        submissions.push((row.stream, row.token));
        assert!(row.frame.is_none(), "unattributed caller timing must stay unknown");
        assert!(row.observed_qpc.is_some());
        match row.payload {
            SourceSubmissionPayload::Software(upload) => software_bytes += upload.bytes,
            SourceSubmissionPayload::Resident => resident += 1,
            SourceSubmissionPayload::Hap { bytes } => assert!(bytes > 0),
        }
    }
    assert_eq!(
        streams.len(),
        6,
        "replacement and separate producers retain distinct lifetimes"
    );
    assert_eq!((software_bytes, resident), (32, 1));
    assert_eq!(recorder.snapshot().dropped_full, 0);
}

use super::*;

fn report(count: usize) -> Report {
    Report {
        before_resource_drop: Snapshot::default(),
        after_resource_drop: Snapshot::default(),
        after_fence: Snapshot::default(),
        operations: Operations::default(),
        completion_markers: 1,
        skipped_completion_markers: 0,
        sample_count: count,
        samples: (0..count)
            .map(|index| Sample {
                elapsed_seconds: index as f64,
                source_phase: None,
                allocations: Snapshot::default(),
                gpu_local_nonlocal: [None, None],
            })
            .collect(),
    }
}

#[test]
fn resource_chunks_preserve_every_snapshot_and_close_exactly_once() {
    let original = report(9);
    let mut stream = Stream::default();
    for message in messages(original) {
        stream.accept(message).unwrap();
    }
    let restored = stream.finish().unwrap();
    assert_eq!(restored.sample_count, 9);
    assert_eq!(restored.samples[8].elapsed_seconds, 8.0);
    assert!(stream.finish().is_err());
    assert!(
        stream
            .accept(Message::Finished {
                report: Box::new(report(0))
            })
            .is_err()
    );
}

#[test]
fn incomplete_reordered_unbounded_or_unretired_resources_do_not_complete() {
    let mut stream = Stream::default();
    assert!(stream.finish().is_err());
    let mut value = report(1);
    assert!(
        stream
            .accept(Message::Samples {
                offset: 1,
                samples: value.samples.clone()
            })
            .is_err()
    );
    value.samples[0].elapsed_seconds = f64::NAN;
    assert!(
        stream
            .accept(Message::Samples {
                offset: 0,
                samples: value.samples
            })
            .is_err()
    );
    let mut source = report(1);
    stream
        .accept(Message::Samples {
            offset: 0,
            samples: std::mem::take(&mut source.samples),
        })
        .unwrap();
    source.after_fence.retiring_textures = 1;
    assert!(
        stream
            .accept(Message::Finished {
                report: Box::new(source)
            })
            .is_err()
    );
    assert!(
        stream
            .accept(Message::Samples {
                offset: 1,
                samples: report(CHUNK_SAMPLES + 1).samples
            })
            .is_err()
    );
}

#[test]
fn explicit_gpu_copy_operations_advance_retirement_without_becoming_sdk_copy_counts() {
    use crate::allocation::{Ledger, Owner, Work};
    let ledger = Ledger::default();
    let texture = ledger.allocate(Owner::Application, 40, 1).unwrap();
    let before = ledger.submitted();
    ledger.work(Work::GpuCopy(40));
    drop(texture);
    ledger.complete(before);
    assert_eq!(ledger.snapshot().retiring_textures, 1);
    ledger.complete(ledger.submitted());
    assert_eq!(ledger.snapshot().retiring_textures, 0);
    assert_eq!(ledger.operations().explicit_gpu_copy_calls, 1);
    assert_eq!(ledger.operations().explicit_gpu_copy_bytes, 40);
    assert_eq!(ledger.operations().sdk_receive_calls, 0);
}

#[test]
fn maximum_admitted_resource_log_cannot_grow_past_its_capacity() {
    let mut stream = Stream::default();
    let mut rejected = 0;
    for message in messages(report(MAX_SAMPLES + 1)) {
        if stream.accept(message).is_err() {
            rejected += 1;
            break;
        }
    }
    assert_eq!(rejected, 1);
    assert!(stream.finish().is_err());
}

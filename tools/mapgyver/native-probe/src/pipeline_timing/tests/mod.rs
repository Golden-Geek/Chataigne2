use super::*;

#[test]
fn dependency_identity_and_absence_survive_without_comparing_gpu_clocks() {
    let mut trace = DependencyTrace::new(10_000_000);
    trace.record(Dependencies {
        renderer_frame: 7,
        sources: [Some(3), None],
    });
    trace.record(Dependencies {
        renderer_frame: 8,
        sources: [Some(3), Some(12)],
    });
    assert_eq!(trace.frames[0].sources, [Some(3), None]);
    assert_eq!(trace.frames[1].sources, [Some(3), Some(12)]);
    assert_eq!(trace.frames[1].renderer_frame, 8);
}

#[test]
fn maximum_session_logs_preserve_their_prefix_and_count_overflow() {
    let mut gpu = GpuTrace::default();
    let mut dependencies = DependencyTrace::new(1);
    for frame in 0..MAX_SAMPLES as u64 + 2 {
        gpu.record(GpuSpan {
            frame,
            submitted_qpc: frame + 1,
            observed_completed_qpc: frame + 2,
            start_ticks: 10,
            end_ticks: 20,
            frequency: 100,
            collected: frame > 30,
        });
        dependencies.record(Dependencies {
            renderer_frame: frame,
            sources: [Some(frame), None],
        });
    }
    assert_eq!(gpu.spans.len(), MAX_SAMPLES);
    assert_eq!(dependencies.frames.len(), MAX_SAMPLES);
    assert_eq!(gpu.spans.last().unwrap().frame, MAX_SAMPLES as u64 - 1);
    assert_eq!(gpu.truncated, 2);
    assert_eq!(dependencies.truncated, 2);
}

use super::*;

use chataigne_alchemist::{LaneRuntimePool, RuntimeDiagnostic, RuntimeIntent};

#[derive(Default)]
struct ScalePassOutput {
    contexts: Vec<ContextKey>,
    intents: Vec<RuntimeIntent>,
    diagnostics: Vec<RuntimeDiagnostic>,
    kernel_ns: u64,
    kernel_evaluations: u64,
}

impl ScalePassOutput {
    fn append(&mut self, mut later: Self) {
        self.contexts.append(&mut later.contexts);
        self.intents.append(&mut later.intents);
        self.diagnostics.append(&mut later.diagnostics);
        self.kernel_ns += later.kernel_ns;
        self.kernel_evaluations += later.kernel_evaluations;
    }
}

fn evaluate_chunk(
    processors: &mut [(Processor, ProcessorRuntime)],
    ctx: &EvaluationCtx<'_>,
    provider: &ScaleContextProvider,
) -> ScalePassOutput {
    let before = chataigne_state_machine::processor_kernel_profile_snapshot();
    let mut output = ScalePassOutput::default();
    for (processor, runtime) in processors {
        let lanes = runtime.evaluate_processor_with_context_provider_and_capture(
            processor,
            ctx,
            provider,
            &ProcessorDebugCapture::Off,
        );
        for lane in lanes {
            output
                .contexts
                .push(lane.context_key.unwrap_or_else(ContextKey::default_lane));
            output.intents.extend(lane.output.intents);
            output.diagnostics.extend(lane.output.diagnostics);
        }
    }
    let after = chataigne_state_machine::processor_kernel_profile_snapshot();
    output.kernel_ns = after.elapsed_ns - before.elapsed_ns;
    output.kernel_evaluations = after.evaluations - before.evaluations;
    output
}

fn evaluate_partition(
    processors: &mut [(Processor, ProcessorRuntime)],
    ctx: &EvaluationCtx<'_>,
    provider: &ScaleContextProvider,
    workers: usize,
) -> ScalePassOutput {
    if workers == 1 {
        return evaluate_chunk(processors, ctx, provider);
    }
    let chunk_size = processors.len().div_ceil(workers);
    std::thread::scope(|scope| {
        let handles = processors
            .chunks_mut(chunk_size)
            .map(|chunk| scope.spawn(move || evaluate_chunk(chunk, ctx, provider)))
            .collect::<Vec<_>>();
        let mut ordered = ScalePassOutput::default();
        for handle in handles {
            ordered.append(handle.join().expect("worker should not panic"));
        }
        ordered
    })
}

fn process_cpu_millis(system: &mut System) -> u64 {
    let pid = get_current_pid().expect("current PID should exist");
    system.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
    system
        .process(pid)
        .expect("current process should exist")
        .accumulated_cpu_time()
}

fn compare_worker_counts(processor_count: usize, lanes_per_processor: usize) {
    let _performance_guard = lock_performance_test();
    let fixture = sample_fixture();
    let provider = ScaleContextProvider::new(lanes_per_processor);
    let registries = RuntimeRegistries {
        value_types: chataigne_state_machine::alchemist::shared_value_type_registry(),
    };
    let mut expected_contexts = None;
    let mut expected_effects: Vec<(Vec<RuntimeIntent>, Vec<RuntimeDiagnostic>)> = Vec::new();
    let mut expected_lane_memory: Option<Vec<LaneRuntimePool>> = None;
    let mut system = System::new();

    for workers in [1, 2, 4, 8] {
        let rss_before_build = resident_bytes(&mut system);
        let mut processors = build_processors(&fixture, &provider, processor_count);
        let mut warmed_tick_us = Vec::new();
        let mut warmed_cpu_ms = Vec::new();
        let mut kernel_ns = 0;
        let mut kernel_evaluations = 0;
        for tick in 1..=4 {
            let ctx = EvaluationCtx {
                logical_tick: tick,
                delta_time: Duration::from_millis(8),
                events: &[],
                inputs: &fixture.inputs,
                registries: &registries,
            };
            let cpu_before = process_cpu_millis(&mut system);
            let started = Instant::now();
            let output = evaluate_partition(&mut processors, &ctx, &provider, workers);
            let elapsed_us = started.elapsed().as_micros();
            let cpu_after = process_cpu_millis(&mut system);
            assert_eq!(output.contexts.len(), 100_000);
            assert!(output.diagnostics.is_empty(), "the real Formula fixture must evaluate without diagnostics");
            if let Some(contexts) = &expected_contexts {
                assert!(
                    output.contexts == *contexts,
                    "context order differs with {workers} workers on tick {tick}"
                );
            } else {
                expected_contexts = Some(output.contexts);
            }
            if workers == 1 {
                expected_effects.push((output.intents, output.diagnostics));
            } else {
                let (intents, diagnostics) = &expected_effects[(tick - 1) as usize];
                assert!(output.intents == *intents, "ordered effects differ with {workers} workers on tick {tick}");
                assert!(
                    output.diagnostics == *diagnostics,
                    "diagnostics differ with {workers} workers on tick {tick}"
                );
            }
            if tick > 1 {
                warmed_tick_us.push(elapsed_us);
                warmed_cpu_ms.push(cpu_after.saturating_sub(cpu_before));
                kernel_ns += output.kernel_ns;
                kernel_evaluations += output.kernel_evaluations;
            }
        }
        assert_eq!(kernel_evaluations, 300_000);
        let memory_count = processors
            .iter()
            .map(|(_, runtime)| runtime.lanes.memory_count())
            .sum::<usize>();
        assert_eq!(memory_count, 100_000);
        let rss_after_evaluation = resident_bytes(&mut system);
        if let Some(expected) = &expected_lane_memory {
            assert_eq!(processors.len(), expected.len());
            assert!(
                processors
                    .iter()
                    .zip(expected)
                    .all(|((_, runtime), reference)| runtime.lanes == *reference),
                "lane memory differs after four ticks with {workers} workers"
            );
        } else {
            expected_lane_memory = Some(processors.iter().map(|(_, runtime)| runtime.lanes.clone()).collect());
        }
        warmed_tick_us.sort_unstable();
        eprintln!(
            "formula workers: processors={processor_count} lanes_per_processor={lanes_per_processor} \
             workers={workers} tick_us={warmed_tick_us:?} process_cpu_ms={warmed_cpu_ms:?} \
             kernel_thread_ms={} kernel_evaluations={kernel_evaluations} ordered_effects={} \
             rss_before_mb={} rss_after_mb={}",
            kernel_ns / 1_000_000,
            expected_effects.iter().map(|(intents, _)| intents.len()).sum::<usize>(),
            rss_before_build / 1_000_000,
            rss_after_evaluation / 1_000_000,
        );
    }
}

#[test]
#[ignore = "manual T18 product-formula worker qualification"]
fn multiplex_formula_workers_1000_by_100() {
    compare_worker_counts(1_000, 100);
}

#[test]
#[ignore = "manual T18 product-formula worker qualification"]
fn multiplex_formula_workers_10000_by_10() {
    compare_worker_counts(10_000, 10);
}

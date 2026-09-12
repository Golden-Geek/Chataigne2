use super::*;

fn profile_unchanged_partition(processor_count: usize, lanes_per_processor: usize) {
    let _performance_guard = lock_performance_test();
    assert_eq!(processor_count * lanes_per_processor, 100_000);
    let fixture = sample_fixture();
    let provider = ScaleContextProvider::new(lanes_per_processor);
    let mut processors = build_processors(&fixture, &provider, processor_count);
    let registries = RuntimeRegistries {
        value_types: chataigne_state_machine::alchemist::shared_value_type_registry(),
    };
    let mut system = System::new();
    let mut warmed_tick_us = Vec::new();
    let mut warmed_cpu_ms = Vec::new();
    let mut warmed_kernel_ns = 0;
    let mut warmed_kernel_evaluations = 0;
    let mut intent_counts = Vec::new();

    for tick in 1..=4 {
        let ctx = EvaluationCtx {
            logical_tick: tick,
            delta_time: Duration::from_millis(8),
            events: &[],
            inputs: &fixture.inputs,
            registries: &registries,
        };
        let kernel_before = chataigne_state_machine::processor_kernel_profile_snapshot();
        let cpu_before = process_cpu_millis(&mut system);
        let started = Instant::now();
        let mut lane_count = 0;
        let mut intent_count = 0;
        for (processor, runtime) in &mut processors {
            let lanes = runtime.evaluate_processor_with_context_provider_and_runtime_delta_capture(
                processor,
                &ctx,
                &provider,
                &ProcessorDebugCapture::Off,
            );
            lane_count += lanes.len();
            for lane in lanes {
                assert!(lane.output.diagnostics.is_empty(), "the product Formula must evaluate without diagnostics");
                intent_count += lane.output.intents.len();
            }
        }
        let elapsed_us = started.elapsed().as_micros();
        let cpu_after = process_cpu_millis(&mut system);
        let kernel_after = chataigne_state_machine::processor_kernel_profile_snapshot();
        assert_eq!(lane_count, 100_000);
        intent_counts.push(intent_count);
        if tick > 1 {
            warmed_tick_us.push(elapsed_us);
            warmed_cpu_ms.push(cpu_after.saturating_sub(cpu_before));
            warmed_kernel_ns += kernel_after.elapsed_ns - kernel_before.elapsed_ns;
            warmed_kernel_evaluations += kernel_after.evaluations - kernel_before.evaluations;
        }
    }

    assert_eq!(warmed_kernel_evaluations, 300_000);
    assert!(intent_counts[0] > 0, "the real Formula must emit intents on initialization");
    assert!(
        intent_counts[1..].iter().all(|count| *count == 0),
        "unchanged inputs must not replay product intents"
    );
    let memory_count = processors
        .iter()
        .map(|(_, runtime)| runtime.lanes.memory_count())
        .sum::<usize>();
    assert_eq!(memory_count, 100_000, "stateful lanes must retain lane-private memory");
    warmed_tick_us.sort_unstable();
    eprintln!(
        "formula unchanged: processors={processor_count} lanes_per_processor={lanes_per_processor} \
         tick_us={warmed_tick_us:?} process_cpu_ms={warmed_cpu_ms:?} \
         kernel_thread_ms={} kernel_evaluations={warmed_kernel_evaluations} \
         intents_per_tick={intent_counts:?} rss_mb={}",
        warmed_kernel_ns / 1_000_000,
        resident_bytes(&mut system) / 1_000_000,
    );
}

#[test]
#[ignore = "manual T18 unchanged-input product-formula qualification"]
fn multiplex_formula_unchanged_1000_by_100() {
    profile_unchanged_partition(1_000, 100);
}

#[test]
#[ignore = "manual T18 unchanged-input product-formula qualification"]
fn multiplex_formula_unchanged_10000_by_10() {
    profile_unchanged_partition(10_000, 10);
}

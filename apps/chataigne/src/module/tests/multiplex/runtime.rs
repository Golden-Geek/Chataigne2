use super::*;

#[test]
fn multiplex_sample_active_runtime_stays_realtime() {
    let _performance_guard = lock_performance_test();
    const SAMPLE: &str = "test_multiplex.noisette";
    const WARMUP: usize = 10;
    const DIRTY_WARMUP: usize = 8;
    // Cover initial bounded log-stream draining and the 200-tick keepalive window.
    const MEASURED: usize = 240;

    let path = targeted_performance_sample_path(SAMPLE);
    let mut engine = load_sparse_project_file::<AppNode, _>(&path).expect("multiplex sample should load");
    let processor_ids = engine
        .nodes
        .iter()
        .filter(|(_, node)| node.get_type() == "state_processor")
        .map(|(_, node)| node.node_data().meta.uuid.0.to_string())
        .collect::<Vec<_>>();
    let processor_count = processor_ids.len();
    assert!(
        processor_count >= 5,
        "the performance regression must exercise at least five sample processors"
    );
    configure_loaded_engine(&mut engine).expect("multiplex sample should configure");
    prepare_engine_for_runtime(&mut engine).expect("multiplex sample should prepare");
    let overview_demand = ProcessorOverviewDemandDto {
        subscription_id: "multiplex-performance-overview".to_owned(),
        processor_ids,
    };
    let manager_id = state_machine_manager_id(&engine);
    let overview_ack = engine.apply_ui_intent(UiEditIntent::SendNodeEvent {
        node: manager_id,
        topic: "chataigne.state_machine.processor_overview_demand".to_owned(),
        payload: serde_json::to_value(overview_demand).expect("overview demand should serialize"),
    });
    assert!(overview_ack.success, "processor overview demand should apply");

    for _ in 0..WARMUP {
        engine
            .run_tick(Duration::from_millis(8))
            .expect("multiplex warmup tick should run");
    }

    let source = multiplex_condition_source(&engine, processor_count);
    let dirty_warmup = measure_multiplex_source_ticks(&mut engine, source, DIRTY_WARMUP, None);
    assert_eq!(
        dirty_warmup.ticks_with_callbacks, DIRTY_WARMUP,
        "dirty-path warmup must exercise scheduled runtime work"
    );
    let read_model = UiReadModel::from_engine(&engine, ProjectFileSpec::new("Noisette", "noisette"));
    let provider_rebuilds_before = context_provider_rebuilds(&engine);
    let debug_samples_before = state_machine_debug_samples_captured(&engine);
    let processor_stats_before = state_machine_processor_runtime_stats(&engine);
    #[cfg(feature = "kernel-profiling")]
    let kernel_before = chataigne_state_machine::processor_kernel_profile_snapshot();
    let measurements = measure_multiplex_source_ticks(&mut engine, source, MEASURED, Some(&read_model));
    let provider_rebuilds_after = context_provider_rebuilds(&engine);
    let debug_samples_after = state_machine_debug_samples_captured(&engine);
    let processor_stats = state_machine_processor_runtime_stats(&engine).since(processor_stats_before);
    #[cfg(feature = "kernel-profiling")]
    let kernel_after = chataigne_state_machine::processor_kernel_profile_snapshot();
    let negative_source_avg_us = measurements.elapsed_us.iter().step_by(2).sum::<u64>() / (MEASURED / 2) as u64;
    let positive_source_avg_us = measurements.elapsed_us.iter().skip(1).step_by(2).sum::<u64>() / (MEASURED / 2) as u64;
    let mut elapsed_us = measurements.elapsed_us;
    elapsed_us.sort_unstable();
    let min_us = elapsed_us[0];
    let max_us = elapsed_us[MEASURED - 1];
    let total_us = elapsed_us.iter().sum::<u64>();
    let avg_us = total_us / MEASURED as u64;
    let p95_us = percentile_us(&elapsed_us, 95);
    let p99_us = percentile_us(&elapsed_us, 99);
    let deadline_misses = elapsed_us.iter().filter(|elapsed| **elapsed >= 10_000).count();
    #[cfg(feature = "kernel-profiling")]
    {
        let kernel_ns = kernel_after.elapsed_ns - kernel_before.elapsed_ns;
        let kernel_evaluations = kernel_after.evaluations - kernel_before.evaluations;
        assert!(kernel_evaluations > 0, "the sample must exercise compiled formula evaluation");
        assert!(kernel_evaluations <= processor_stats.lanes_evaluated);
        eprintln!(
            "multiplex kernel: elapsed_us={} evaluations={} tick_share_pct={:.1} evaluation_phase_share_pct={:.1}",
            kernel_ns / 1_000,
            kernel_evaluations,
            kernel_ns as f64 / (total_us * 1_000) as f64 * 100.0,
            kernel_ns as f64 / processor_stats.evaluation_ns as f64 * 100.0,
        );
    }
    let mut published_elapsed_us = measurements.published_elapsed_us;
    published_elapsed_us.sort_unstable();
    let published_avg_us = published_elapsed_us.iter().sum::<u64>() / published_elapsed_us.len() as u64;
    let published_p95_us = percentile_us(&published_elapsed_us, 95);
    let published_p99_us = percentile_us(&published_elapsed_us, 99);
    let published_deadline_misses = published_elapsed_us
        .iter()
        .filter(|elapsed| **elapsed >= 10_000)
        .count();
    eprintln!(
        concat!(
            "multiplex runtime: avg={avg_us}us negative_avg={negative_source_avg_us}us ",
            "positive_avg={positive_source_avg_us}us p95={p95_us}us p99={p99_us}us ",
            "min={min_us}us max={max_us}us deadline_misses={deadline_misses} ",
            "published_avg={published_avg_us}us published_p95={published_p95_us}us ",
            "published_p99={published_p99_us}us published_deadline_misses={published_deadline_misses} ",
            "published_events={} callbacks={} callback_ticks={} snapshot_builds={} ",
            "provider_rebuilds={} budget_rejected_actions={} budget_rejected_intents={} ",
            "processor_input_us={} processor_eval_us={} processor_eval_calls={} ",
            "processor_lanes={} processor_eval_tick_share_pct={:.1}"
        ),
        measurements.published_events,
        measurements.callbacks_fired,
        measurements.ticks_with_callbacks,
        measurements.snapshot_builds,
        provider_rebuilds_after - provider_rebuilds_before,
        processor_stats.budget_rejected_actions,
        processor_stats.budget_rejected_intents,
        processor_stats.input_preparation_ns / 1_000,
        processor_stats.evaluation_ns / 1_000,
        processor_stats.evaluation_calls,
        processor_stats.lanes_evaluated,
        processor_stats.evaluation_ns as f64 / (total_us * 1_000) as f64 * 100.0,
        avg_us = avg_us,
        negative_source_avg_us = negative_source_avg_us,
        positive_source_avg_us = positive_source_avg_us,
        p95_us = p95_us,
        p99_us = p99_us,
        min_us = min_us,
        max_us = max_us,
        deadline_misses = deadline_misses,
        published_avg_us = published_avg_us,
        published_p95_us = published_p95_us,
        published_p99_us = published_p99_us,
        published_deadline_misses = published_deadline_misses,
    );
    assert!(processor_stats.evaluation_calls > 0 && processor_stats.lanes_evaluated > 0);
    assert_eq!(
        measurements.ticks_with_callbacks, MEASURED,
        "every dirty measured tick must execute scheduled runtime work"
    );
    assert_eq!(
        measurements.snapshot_builds, 0,
        "all steady multiplex ticks must reuse the state runtime snapshot"
    );
    assert_eq!(
        debug_samples_after, debug_samples_before,
        "the all-processor overview must not enable Alchemist debug capture"
    );
    assert!(
        processor_stats.batched_executions > 0,
        "the sample's output containers should use ordered command batches"
    );
    assert!(
        processor_stats.command_batches.saturating_mul(8) < processor_stats.batched_executions,
        "command batching must collapse lane fan-out: {} batches for {} executions",
        processor_stats.command_batches,
        processor_stats.batched_executions,
    );
    assert_eq!(
        processor_stats.rejected_executions, 0,
        "the checked multiplex sample must not emit non-finite command overrides"
    );
    assert_eq!(
        (processor_stats.budget_rejected_actions, processor_stats.budget_rejected_intents,),
        (0, 0),
        "the checked multiplex sample must fit within the explicit per-tick command budget"
    );
    assert!(
        measurements.published_events >= MEASURED,
        "every dirty tick must publish at least its source-value event"
    );
    if strict_serial_performance_assertions() {
        assert!(
            avg_us < 5_000,
            "serial multiplex runtime averaged {avg_us}us per dirty tick; the 200 Hz development budget is 5000us"
        );
        assert!(
            p95_us < 10_000,
            "serial multiplex runtime p95 reached {p95_us}us"
        );
        assert!(
            p99_us < 10_000,
            "serial multiplex runtime p99 reached {p99_us}us; dirty compute must stay inside the 100 Hz deadline"
        );
        assert!(
            deadline_misses <= 2,
            "serial multiplex runtime missed the 100 Hz deadline {deadline_misses} times; at most two host-deschedule outliers are allowed"
        );
        assert!(
            published_avg_us < 6_000,
            "serial multiplex runtime plus incremental UI publication averaged {published_avg_us}us; the full dev-host path must stay responsive"
        );
        assert!(
            published_p95_us < 10_000,
            "serial multiplex runtime plus incremental UI publication p95 reached {published_p95_us}us"
        );
        assert!(
            published_p99_us < 10_000,
            "serial multiplex runtime plus incremental UI publication p99 reached {published_p99_us}us"
        );
        assert!(
            published_deadline_misses <= 2,
            "serial multiplex runtime plus UI publication missed the 100 Hz deadline {published_deadline_misses} times"
        );
    }
}

#[test]
fn multiplex_sample_production_runtime_stays_realtime() {
    let _performance_guard = lock_performance_test();
    const SAMPLE: &str = "test_multiplex.noisette";
    const WARMUP: usize = 10;
    const MEASURED: usize = 240;

    let path = targeted_performance_sample_path(SAMPLE);
    let mut engine = load_sparse_project_file::<AppNode, _>(&path).expect("multiplex sample should load");
    let processor_ids = engine
        .nodes
        .iter()
        .filter(|(_, node)| node.get_type() == "state_processor")
        .map(|(_, node)| node.node_data().meta.uuid.0.to_string())
        .collect::<Vec<_>>();
    let processor_count = processor_ids.len();
    assert!(
        processor_count >= 5,
        "the production regression must exercise at least five sample processors"
    );
    configure_loaded_engine(&mut engine).expect("multiplex sample should configure");
    prepare_engine_for_runtime(&mut engine).expect("multiplex sample should prepare");
    let manager_id = state_machine_manager_id(&engine);
    let overview_ack = engine.apply_ui_intent(UiEditIntent::SendNodeEvent {
        node: manager_id,
        topic: "chataigne.state_machine.processor_overview_demand".to_owned(),
        payload: serde_json::to_value(ProcessorOverviewDemandDto {
            subscription_id: "multiplex-production-performance-overview".to_owned(),
            processor_ids,
        })
        .expect("overview demand should serialize"),
    });
    assert!(overview_ack.success, "processor overview demand should apply");
    let source = multiplex_condition_source(&engine, processor_count);
    let runtime = ProductionRuntime::new(
        engine,
        UiProjectFileSpec::from_project_file_spec(ProjectFileSpec::new("Noisette", "noisette"), None),
    );
    let input = runtime.input_port();

    for index in 0..WARMUP {
        input
            .publish(
                source,
                ParamValue::Float(if index % 2 == 0 { -1.0 } else { 2.0 }),
                index as u64 + 1,
            )
            .expect("multiplex warmup input should publish");
        runtime
            .run_tick(Duration::from_millis(8))
            .expect("multiplex warmup tick should run");
    }

    let mut elapsed_us = Vec::with_capacity(MEASURED);
    let mut published_events = 0usize;
    for index in 0..MEASURED {
        input
            .publish(
                source,
                ParamValue::Float(if index % 2 == 0 { -1.0 } else { 2.0 }),
                (WARMUP + index) as u64 + 1,
            )
            .expect("multiplex measured input should publish");
        let started = Instant::now();
        let result = runtime
            .run_tick(Duration::from_millis(8))
            .expect("multiplex measured tick should run");
        elapsed_us.push(started.elapsed().as_micros() as u64);
        published_events += result.events.events.len();
    }

    elapsed_us.sort_unstable();
    let avg_us = elapsed_us.iter().sum::<u64>() / MEASURED as u64;
    let p95_us = percentile_us(&elapsed_us, 95);
    let p99_us = percentile_us(&elapsed_us, 99);
    let min_us = elapsed_us[0];
    let max_us = elapsed_us[MEASURED - 1];
    let deadline_misses = elapsed_us.iter().filter(|elapsed| **elapsed >= 10_000).count();
    eprintln!(
        "multiplex production runtime: avg={avg_us}us p95={p95_us}us p99={p99_us}us min={min_us}us max={max_us}us deadline_misses={deadline_misses} published_events={published_events}"
    );

    assert!(
        published_events >= MEASURED,
        "every production tick must publish at least its source-value event"
    );
    if strict_serial_performance_assertions() {
        assert!(
            avg_us < 6_000,
            "serial production runtime averaged {avg_us}us; the full control/read-model path must stay responsive"
        );
        assert!(
            p95_us < 10_000,
            "serial production runtime p95 reached {p95_us}us"
        );
        assert!(
            p99_us < 10_000,
            "serial production runtime p99 reached {p99_us}us; the full path must stay inside the 100 Hz deadline"
        );
        assert!(
            deadline_misses <= 2,
            "serial production runtime missed the 100 Hz deadline {deadline_misses} times"
        );
    }
}

use std::{
    hint::black_box,
    time::{Duration, Instant},
};

use chataigne_alchemist::{
    ANodeDeclaration, ANodeInstance, AlchemistFormula, AlchemistGraphDomain, CompileCtx, ContextKey, EvaluationCtx,
    FormulaContextContract, FormulaId, FormulaPropertySchema, FormulaSurface, ManagedFilterValueMode, ManagedItemId,
    ManagedItemInstance, ManagedItemUiState, ManagedRegionDefinition, ManagedRegionId, ManagedRegionInstance,
    ManagedRegionKind, PrimitiveNodeDeclaration, PrimitiveNodeKind, RuntimeInputSnapshot, RuntimeRegistries, SocketId,
    StableRef, SurfaceItemKind, ValueTypeId, ValueTypeRegistry,
};
use chataigne_processor::{
    ChannelSourceSchema, INPUT_SOURCE_FIELD, ManagedFormulaRuntime, ManagedStageSpecializationCache,
    OUTPUT_BINDINGS_FIELD, OUTPUT_TARGET_FIELD, OutputBindingConfig, OutputValueSource, RuntimeInputBinding,
    ValueLaneKey, alchemist::node_registry,
};
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use golden_values::Value as RuntimeValue;
use indexmap::IndexSet;

fn mapping_runtime(c: &mut Criterion) {
    let mut group = c.benchmark_group("mapping_runtime_float");
    for (sources, depth) in [(1, 1), (8, 1), (32, 1), (8, 8), (32, 8)] {
        let (mut runtimes, inputs, value_types) = build_case(1, sources, depth, Workload::NumericChain);
        let registries = RuntimeRegistries {
            value_types: &value_types,
        };
        let context = evaluation_context(&inputs, &registries);
        assert_eq!(evaluate_batch(&mut runtimes, &context), 1);

        group.throughput(Throughput::Elements(sources as u64));
        group.bench_function(BenchmarkId::new(format!("depth_{depth}"), sources), |bench| {
            bench.iter(|| black_box(evaluate_batch(&mut runtimes, black_box(&context))));
        });
    }
    group.finish();
}

fn mapping_runtime_batch(c: &mut Criterion) {
    let mut group = c.benchmark_group("mapping_runtime_processor_batch");
    for (processors, sources, depth) in [(1_000, 1, 1), (10_000, 1, 1), (1_000, 8, 8)] {
        let (mut runtimes, inputs, value_types) = build_case(processors, sources, depth, Workload::NumericChain);
        let registries = RuntimeRegistries {
            value_types: &value_types,
        };
        let context = evaluation_context(&inputs, &registries);
        assert_eq!(evaluate_batch(&mut runtimes, &context), processors);

        group.throughput(Throughput::Elements((processors * sources) as u64));
        group.bench_function(
            BenchmarkId::new(format!("sources_{sources}_depth_{depth}"), processors),
            |bench| {
                bench.iter(|| black_box(evaluate_batch(&mut runtimes, black_box(&context))));
            },
        );
    }
    group.finish();
}

fn mapping_runtime_shapes(c: &mut Criterion) {
    let mut group = c.benchmark_group("mapping_runtime_shapes_1000");
    for (label, workload) in [
        ("sum_3", Workload::Sum),
        ("pack_vec3", Workload::PackVec3),
        ("mixed_passthrough_3", Workload::MixedPassthrough),
    ] {
        let (mut runtimes, inputs, value_types) = build_case(1_000, 3, 1, workload);
        let registries = RuntimeRegistries {
            value_types: &value_types,
        };
        let context = evaluation_context(&inputs, &registries);
        assert_eq!(evaluate_batch(&mut runtimes, &context), 1_000);
        group.throughput(Throughput::Elements(3_000));
        group.bench_function(label, |bench| {
            bench.iter(|| black_box(evaluate_batch(&mut runtimes, black_box(&context))));
        });
    }
    group.finish();
}

fn mapping_runtime_contexts(c: &mut Criterion) {
    let (mut runtimes, inputs, value_types) = build_case(1, 8, 8, Workload::NumericChain);
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let context = evaluation_context(&inputs, &registries);
    let keys = (0..8)
        .map(|index| ContextKey::single("benchmark", format!("context_{index}")))
        .collect::<Vec<_>>();
    let runtime = &mut runtimes[0];
    assert_eq!(evaluate_contexts(runtime, &context, &keys), 8);
    let mut group = c.benchmark_group("mapping_runtime_contexts");
    group.throughput(Throughput::Elements(64));
    group.bench_function("eight_contexts_eight_sources_eight_stages", |bench| {
        bench.iter(|| black_box(evaluate_contexts(runtime, black_box(&context), &keys)));
    });
    group.finish();
}

fn mapping_runtime_latency_distribution(c: &mut Criterion) {
    let Some(sample_count) = std::env::var("CHATAIGNE_MAPPING_LATENCY_SAMPLES")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
    else {
        return;
    };
    assert!(sample_count >= 100, "latency distribution needs at least 100 samples");

    for (label, processors, sources, depth, workload, baseline_p95_ns) in [
        ("scalar_1000", 1_000, 1, 1, Workload::NumericChain, 1_546_000),
        ("scalar_10000", 10_000, 1, 1, Workload::NumericChain, 41_880_000),
        ("tuple_1000_8x8", 1_000, 8, 8, Workload::NumericChain, 89_707_000),
        ("sum_1000", 1_000, 3, 1, Workload::Sum, 2_065_000),
        ("pack_vec3_1000", 1_000, 3, 1, Workload::PackVec3, 2_110_000),
        ("mixed_1000", 1_000, 3, 1, Workload::MixedPassthrough, 1_162_000),
    ] {
        let (mut runtimes, inputs, value_types) = build_case(processors, sources, depth, workload);
        let registries = RuntimeRegistries {
            value_types: &value_types,
        };
        let context = evaluation_context(&inputs, &registries);
        for _ in 0..16 {
            black_box(evaluate_batch(&mut runtimes, &context));
        }
        let mut samples = Vec::with_capacity(sample_count);
        for _ in 0..sample_count {
            let start = Instant::now();
            black_box(evaluate_batch(&mut runtimes, black_box(&context)));
            samples.push(start.elapsed().as_nanos() as u64);
        }
        let p95 = report_latency_distribution(label, &mut samples);
        assert_baseline_p95(label, p95, baseline_p95_ns);
    }

    let (mut runtimes, inputs, value_types) = build_case(1, 8, 8, Workload::NumericChain);
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let context = evaluation_context(&inputs, &registries);
    let keys = (0..8)
        .map(|index| ContextKey::single("benchmark", format!("context_{index}")))
        .collect::<Vec<_>>();
    for _ in 0..16 {
        black_box(evaluate_contexts(&mut runtimes[0], &context, &keys));
    }
    let mut samples = Vec::with_capacity(sample_count);
    for _ in 0..sample_count {
        let start = Instant::now();
        black_box(evaluate_contexts(&mut runtimes[0], black_box(&context), &keys));
        samples.push(start.elapsed().as_nanos() as u64);
    }
    let p95 = report_latency_distribution("contexts_8x8x8", &mut samples);
    assert_baseline_p95("contexts_8x8x8", p95, 440_000);

    let (mut runtimes, inputs, value_types) = build_case(1_000, 1, 1, Workload::NumericChain);
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let context = evaluation_context(&inputs, &registries);
    let mut group = c.benchmark_group("mapping_runtime_latency_distribution");
    group.bench_function("scalar_1000_reference", |bench| {
        bench.iter(|| black_box(evaluate_batch(&mut runtimes, black_box(&context))));
    });
    group.finish();
}

fn mapping_runtime_allocation_report(c: &mut Criterion) {
    if std::env::var_os("CHATAIGNE_MAPPING_ALLOCATION_REPORT").is_none() {
        return;
    }
    let mut reference_allocations_per_processor = None;
    for (label, processors, sources, depth, workload) in [
        ("scalar_1_depth_1", 1, 1, 1, Workload::NumericChain),
        ("tuple_8_depth_1", 1, 8, 1, Workload::NumericChain),
        ("tuple_32_depth_1", 1, 32, 1, Workload::NumericChain),
        ("tuple_8_depth_8", 1, 8, 8, Workload::NumericChain),
        ("tuple_32_depth_8", 1, 32, 8, Workload::NumericChain),
        ("scalar_1000", 1_000, 1, 1, Workload::NumericChain),
        ("sum_1000", 1_000, 3, 1, Workload::Sum),
        ("pack_vec3_1000", 1_000, 3, 1, Workload::PackVec3),
        ("mixed_1000", 1_000, 3, 1, Workload::MixedPassthrough),
    ] {
        let (mut runtimes, inputs, value_types) = build_case(processors, sources, depth, workload);
        let registries = RuntimeRegistries {
            value_types: &value_types,
        };
        let context = evaluation_context(&inputs, &registries);
        for _ in 0..16 {
            black_box(evaluate_batch(&mut runtimes, &context));
        }
        let allocations = allocation_counter::measure(|| {
            black_box(evaluate_batch(&mut runtimes, black_box(&context)));
        });
        let reference = *reference_allocations_per_processor.get_or_insert(allocations.count_total);
        assert_eq!(
            allocations.count_total,
            reference * u64::try_from(processors).expect("benchmark processor count fits in u64"),
            "{label} introduced tuple-element or stage allocations"
        );
        assert_eq!(
            allocations.count_current, 0,
            "{label} retained allocations in one warmed evaluation"
        );
        println!(
            "mapping_allocations {label} count_total={} bytes_total={} count_current={} bytes_current={}",
            allocations.count_total, allocations.bytes_total, allocations.count_current, allocations.bytes_current,
        );
    }

    let (mut runtimes, inputs, value_types) = build_case(1_000, 1, 1, Workload::NumericChain);
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let context = evaluation_context(&inputs, &registries);
    let mut group = c.benchmark_group("mapping_runtime_allocation_report");
    group.bench_function("scalar_1000_reference", |bench| {
        bench.iter(|| black_box(evaluate_batch(&mut runtimes, black_box(&context))));
    });
    group.finish();
}

fn mapping_runtime_activity_distribution(c: &mut Criterion) {
    let Some(sample_count) = std::env::var("CHATAIGNE_MAPPING_ACTIVITY_SAMPLES")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
    else {
        return;
    };
    assert!(sample_count >= 100, "activity distribution needs at least 100 samples");

    let (mut runtimes, mut inputs, value_types) = build_case(1, 1, 1, Workload::NumericChain);
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let source = source_reference(0, 0);
    report_activity("source_change_1", sample_count, 3_000, |tick| {
        inputs.insert(
            source.clone(),
            RuntimeValue::Float(if tick % 2 == 0 { 0.25 } else { 0.75 }),
        );
        let context = evaluation_context_at(&inputs, &registries, tick as u64 + 1);
        let output = runtimes[0].evaluate(&context);
        assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
        (output.intents.len(), output.debug_samples.len())
    });

    let (mut runtimes, mut inputs, value_types, filter) = build_case_with_first_filter(1, 1, 1, Workload::NumericChain);
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    inputs.insert(source_reference(0, 0), RuntimeValue::Float(0.5));
    let filter = filter.expect("Remap benchmark has one filter");
    let socket = SocketId::new("out_max");
    report_activity("runtime_setting_1", sample_count, 3_000, |tick| {
        let upper = if tick % 2 == 0 { 0.25 } else { 0.75 };
        runtimes[0]
            .update_filter_input(
                filter,
                &socket,
                RuntimeInputBinding::Constant(RuntimeValue::Float(upper)),
            )
            .expect("live Remap setting should update without compilation");
        let context = evaluation_context_at(&inputs, &registries, tick as u64 + 1);
        let output = runtimes[0].evaluate(&context);
        assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
        assert_eq!(output.intents[0].payload, RuntimeValue::Float(upper * 0.5));
        (output.intents.len(), output.debug_samples.len())
    });

    let (mut runtimes, mut inputs, value_types) = build_case(1, 1, 1, Workload::Smooth);
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    assert!(runtimes[0].needs_continuous_evaluation());
    report_activity("temporal_smooth_1", sample_count, 3_000, |tick| {
        if tick == 16 {
            inputs.insert(source_reference(0, 0), RuntimeValue::Float(1.0));
        }
        let context = evaluation_context_at(&inputs, &registries, tick as u64 + 1);
        let output = runtimes[0].evaluate(&context);
        assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
        (output.intents.len(), output.debug_samples.len())
    });

    let (mut runtimes, mut inputs, value_types) = build_case(1, 8, 8, Workload::NumericChain);
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let source = source_reference(0, 0);
    report_activity("preview_8x8", sample_count, 100_000, |tick| {
        inputs.insert(
            source.clone(),
            RuntimeValue::Float(if tick % 2 == 0 { 0.25 } else { 0.75 }),
        );
        let context = evaluation_context_at(&inputs, &registries, tick as u64 + 1);
        let output = runtimes[0].evaluate_with_context_frame(
            &context,
            &ContextKey::default_lane(),
            None,
            chataigne_alchemist::DebugCaptureMode::All { history_len: 1 },
        );
        assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
        (output.intents.len(), output.debug_samples.len())
    });

    report_context_cleanup(sample_count);

    let (mut runtimes, inputs, value_types) = build_case(1, 1, 1, Workload::NumericChain);
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let context = evaluation_context(&inputs, &registries);
    let mut group = c.benchmark_group("mapping_runtime_activity_distribution");
    group.bench_function("scalar_reference", |bench| {
        bench.iter(|| black_box(evaluate_batch(&mut runtimes, black_box(&context))));
    });
    group.finish();
}

fn report_activity(
    label: &str,
    sample_count: usize,
    baseline_p95_ns: u64,
    mut evaluate: impl FnMut(usize) -> (usize, usize),
) {
    for tick in 0..16 {
        black_box(evaluate(tick));
    }
    let mut samples = Vec::with_capacity(sample_count);
    let (mut intents, mut previews) = (0, 0);
    for sample in 0..sample_count {
        let start = Instant::now();
        let (sample_intents, sample_previews) = black_box(evaluate(sample + 16));
        samples.push(start.elapsed().as_nanos() as u64);
        intents += sample_intents;
        previews += sample_previews;
    }
    println!("mapping_activity {label} samples={sample_count} intents={intents} previews={previews}");
    let p95 = report_latency_distribution(label, &mut samples);
    assert_baseline_p95(label, p95, baseline_p95_ns);
}

fn report_context_cleanup(sample_count: usize) {
    let (mut runtimes, inputs, value_types) = build_case(1, 1, 1, Workload::Smooth);
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let context = evaluation_context(&inputs, &registries);
    let keys = (0..128)
        .map(|index| ContextKey::single("benchmark", format!("context_{index}")))
        .collect::<Vec<_>>();
    let keep = keys.iter().take(8).cloned().collect::<IndexSet<_>>();
    let runtime = &mut runtimes[0];
    let mut samples = Vec::with_capacity(sample_count);
    for cycle in 0..sample_count + 16 {
        for key in &keys {
            let output =
                runtime.evaluate_with_context_frame(&context, key, None, chataigne_alchemist::DebugCaptureMode::Off);
            assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
        }
        assert_eq!(runtime.retained_state_lane_count(), keys.len());
        let start = Instant::now();
        runtime.retain_context_keys(&keep);
        let elapsed = start.elapsed().as_nanos() as u64;
        assert_eq!(runtime.retained_state_lane_count(), keep.len());
        if cycle >= 16 {
            samples.push(elapsed);
        }
    }
    println!("mapping_retained_state before={} after={}", keys.len(), keep.len());
    let p95 = report_latency_distribution("context_cleanup_128_to_8", &mut samples);
    assert_baseline_p95("context_cleanup_128_to_8", p95, 100_000);
}

fn report_latency_distribution(label: &str, samples: &mut [u64]) -> u64 {
    samples.sort_unstable();
    let percentile = |percent: usize| samples[(samples.len() * percent).div_ceil(100) - 1];
    let p95 = percentile(95);
    println!(
        "mapping_latency {label} samples={} p50_ns={} p95_ns={} p99_ns={}",
        samples.len(),
        percentile(50),
        p95,
        percentile(99),
    );
    p95
}

fn assert_baseline_p95(label: &str, measured_ns: u64, baseline_ns: u64) {
    if std::env::var_os("CHATAIGNE_MAPPING_ENFORCE_275HX_BASELINE").is_some() {
        assert!(
            measured_ns <= baseline_ns,
            "{label} p95 {measured_ns} ns exceeded recorded 275HX baseline {baseline_ns} ns"
        );
    }
}

fn evaluation_context<'a>(
    inputs: &'a RuntimeInputSnapshot,
    registries: &'a RuntimeRegistries<'a>,
) -> EvaluationCtx<'a> {
    evaluation_context_at(inputs, registries, 1)
}

fn evaluation_context_at<'a>(
    inputs: &'a RuntimeInputSnapshot,
    registries: &'a RuntimeRegistries<'a>,
    logical_tick: u64,
) -> EvaluationCtx<'a> {
    EvaluationCtx {
        logical_tick,
        delta_time: Duration::from_millis(16),
        events: &[],
        inputs,
        registries,
    }
}

fn source_reference(processor: usize, index: usize) -> StableRef {
    StableRef::new(
        ValueTypeId::new("chataigne.module_endpoint"),
        format!("source/{processor}/{index}"),
    )
}

fn evaluate_batch(runtimes: &mut [ManagedFormulaRuntime], context: &EvaluationCtx<'_>) -> usize {
    runtimes
        .iter_mut()
        .map(|runtime| {
            let output = runtime.evaluate(context);
            assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
            output.intents.len()
        })
        .sum()
}

fn evaluate_contexts(runtime: &mut ManagedFormulaRuntime, context: &EvaluationCtx<'_>, keys: &[ContextKey]) -> usize {
    keys.iter()
        .map(|key| {
            let output =
                runtime.evaluate_with_context_frame(context, key, None, chataigne_alchemist::DebugCaptureMode::Off);
            assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
            output.intents.len()
        })
        .sum()
}

#[derive(Clone, Copy, Debug)]
enum Workload {
    NumericChain,
    Sum,
    PackVec3,
    MixedPassthrough,
    Smooth,
}

fn build_case(
    processor_count: usize,
    source_count: usize,
    depth: usize,
    workload: Workload,
) -> (Vec<ManagedFormulaRuntime>, RuntimeInputSnapshot, ValueTypeRegistry) {
    let (runtimes, inputs, value_types, _) =
        build_case_with_first_filter(processor_count, source_count, depth, workload);
    (runtimes, inputs, value_types)
}

fn build_case_with_first_filter(
    processor_count: usize,
    source_count: usize,
    depth: usize,
    workload: Workload,
) -> (
    Vec<ManagedFormulaRuntime>,
    RuntimeInputSnapshot,
    ValueTypeRegistry,
    Option<ManagedItemId>,
) {
    let formula = formula();
    let value_types = chataigne_processor::alchemist::value_type_registry();
    let nodes = node_registry();
    let compile_ctx = CompileCtx {
        value_types: &value_types,
        nodes: &nodes,
        properties: Some(&formula.properties),
    };
    let mut inputs = RuntimeInputSnapshot::default();
    let mut runtimes = Vec::with_capacity(processor_count);
    let mut cache = ManagedStageSpecializationCache::default();
    let mut first_filter = None;
    for processor in 0..processor_count {
        let mut instance = formula.instantiate();
        let sources: Vec<_> = (0..source_count)
            .map(|index| {
                let source = source_reference(processor, index);
                let value = match (workload, index) {
                    (Workload::MixedPassthrough, 1) => RuntimeValue::Bool(true),
                    (Workload::MixedPassthrough, 2) => RuntimeValue::String("benchmark".into()),
                    _ => RuntimeValue::Float(index as f64 / source_count as f64),
                };
                inputs.insert(source.clone(), value);
                input_item(source)
            })
            .collect();
        let first_input = ValueLaneKey::input(sources[0].id);
        instance
            .managed_regions
            .regions
            .insert(ManagedRegionId::new("inputs"), region("inputs", sources));
        let filters = match workload {
            Workload::NumericChain => (0..depth)
                .map(|index| if index % 2 == 0 { remap() } else { clamp() })
                .collect(),
            Workload::Sum => {
                let mut stage = item(PrimitiveNodeKind::Sum);
                stage.anode.config.set("num_inputs", RuntimeValue::Int(3));
                vec![stage]
            }
            Workload::PackVec3 => vec![item(PrimitiveNodeKind::PackVec3)],
            Workload::MixedPassthrough => Vec::new(),
            Workload::Smooth => vec![item(PrimitiveNodeKind::SmoothFilter)],
        };
        if first_filter.is_none() {
            first_filter = filters.first().map(|filter| filter.id);
        }
        instance
            .managed_regions
            .regions
            .insert(ManagedRegionId::new("filters"), region("filters", filters));
        instance.managed_regions.regions.insert(
            ManagedRegionId::new("outputs"),
            region(
                "outputs",
                vec![output_item(
                    processor,
                    matches!(workload, Workload::NumericChain | Workload::MixedPassthrough) && source_count > 1,
                    first_input,
                )],
            ),
        );
        let mut runtime = ManagedFormulaRuntime::compile(&formula, &instance, &compile_ctx)
            .expect("benchmark Formula should compile")
            .expect("benchmark Formula should have managed regions");
        runtime
            .reconcile_input_source_schema_with_cache(
                |reference| {
                    let index = reference
                        .stable_id
                        .rsplit('/')
                        .next()
                        .and_then(|value| value.parse::<usize>().ok())
                        .unwrap();
                    let value_type = match (workload, index) {
                        (Workload::MixedPassthrough, 1) => "bool",
                        (Workload::MixedPassthrough, 2) => "string",
                        _ => "float",
                    };
                    Some(ChannelSourceSchema {
                        value_type: ValueTypeId::new(value_type),
                        metadata: Default::default(),
                    })
                },
                Some(&mut cache),
            )
            .expect("benchmark sources should resolve");
        runtimes.push(runtime);
    }
    let expected_plans = match workload {
        Workload::NumericChain if depth > 1 => 2,
        Workload::MixedPassthrough => 0,
        _ => 1,
    };
    assert_eq!(
        cache.len(),
        expected_plans,
        "equivalent Mapping stages should share compiled plans"
    );
    if std::env::var_os("CHATAIGNE_MAPPING_CACHE_REPORT").is_some() {
        println!(
            "mapping_cache processors={processor_count} sources={source_count} depth={depth} workload={workload:?} unique_stage_plans={}",
            cache.len()
        );
    }
    (runtimes, inputs, value_types, first_filter)
}

fn formula() -> AlchemistFormula {
    AlchemistFormula {
        id: FormulaId::new("bench.mapping"),
        version: 1,
        label: "Benchmark Mapping".into(),
        description: None,
        tags: Vec::new(),
        graph: AlchemistGraphDomain::new_document(),
        properties: FormulaPropertySchema::default(),
        surface: FormulaSurface {
            sections: Vec::new(),
            managed_regions: [
                ("inputs", ManagedRegionKind::InputSet, SurfaceItemKind::Input),
                ("filters", ManagedRegionKind::FilterPipeline, SurfaceItemKind::Filter),
                ("outputs", ManagedRegionKind::OutputSet, SurfaceItemKind::Output),
            ]
            .into_iter()
            .map(|(id, kind, role)| ManagedRegionDefinition {
                id: ManagedRegionId::new(id),
                kind,
                label: id.into(),
                input_socket: None,
                output_socket: None,
                accepted_roles: vec![role],
                filter_value_mode: ManagedFilterValueMode::Tuple,
            })
            .collect(),
        },
        context_contract: FormulaContextContract::default(),
        migrations: Vec::new(),
    }
}

fn region(id: &str, items: Vec<ManagedItemInstance>) -> ManagedRegionInstance {
    ManagedRegionInstance {
        region_id: ManagedRegionId::new(id),
        items,
    }
}

fn item(kind: PrimitiveNodeKind) -> ManagedItemInstance {
    let declaration = PrimitiveNodeDeclaration::new(kind);
    managed_item(ANodeInstance::new(declaration.type_id(), declaration.label()))
}

fn managed_item(anode: ANodeInstance) -> ManagedItemInstance {
    ManagedItemInstance {
        id: ManagedItemId::new(),
        anode,
        enabled: true,
        ui_state: ManagedItemUiState::default(),
    }
}

fn input_item(source: StableRef) -> ManagedItemInstance {
    let mut anode = ANodeInstance::new("managed_input".into(), "Input");
    anode.config.set(INPUT_SOURCE_FIELD, RuntimeValue::Ref(source));
    managed_item(anode)
}

fn output_item(processor: usize, select_first: bool, first_input: ValueLaneKey) -> ManagedItemInstance {
    let mut anode = ANodeInstance::new("managed_output".into(), "Output");
    anode.config.set(
        OUTPUT_TARGET_FIELD,
        RuntimeValue::Ref(StableRef::new(
            ValueTypeId::new("chataigne.command_target"),
            format!("target/{processor}"),
        )),
    );
    if select_first {
        anode.config.set(
            OUTPUT_BINDINGS_FIELD,
            OutputBindingConfig {
                value: OutputValueSource::Element(first_input),
                ..OutputBindingConfig::default()
            }
            .to_runtime_value()
            .expect("benchmark output bindings should encode"),
        );
    }
    managed_item(anode)
}

fn remap() -> ManagedItemInstance {
    let mut item = item(PrimitiveNodeKind::Remap);
    for (socket, value) in [("in_min", 0.0), ("in_max", 1.0), ("out_min", 0.0), ("out_max", 1.0)] {
        item.anode
            .input_defaults
            .insert(SocketId::new(socket), RuntimeValue::Float(value));
    }
    item
}

fn clamp() -> ManagedItemInstance {
    let mut item = item(PrimitiveNodeKind::Clamp);
    for (socket, value) in [("minimum", 0.0), ("maximum", 1.0)] {
        item.anode
            .input_defaults
            .insert(SocketId::new(socket), RuntimeValue::Float(value));
    }
    item
}

criterion_group!(
    benches,
    mapping_runtime,
    mapping_runtime_batch,
    mapping_runtime_shapes,
    mapping_runtime_contexts,
    mapping_runtime_latency_distribution,
    mapping_runtime_allocation_report,
    mapping_runtime_activity_distribution
);
criterion_main!(benches);

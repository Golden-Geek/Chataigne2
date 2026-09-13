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
    ChannelSourceSchema, INPUT_SOURCE_FIELD, ManagedFormulaRuntime, OUTPUT_BINDINGS_FIELD, OUTPUT_TARGET_FIELD,
    OutputBindingConfig, OutputValueSource, ValueLaneKey, alchemist::node_registry,
};
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use golden_values::Value as RuntimeValue;

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

    for (label, processors, sources, depth, workload) in [
        ("scalar_1000", 1_000, 1, 1, Workload::NumericChain),
        ("scalar_10000", 10_000, 1, 1, Workload::NumericChain),
        ("tuple_1000_8x8", 1_000, 8, 8, Workload::NumericChain),
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
        let mut samples = Vec::with_capacity(sample_count);
        for _ in 0..sample_count {
            let start = Instant::now();
            black_box(evaluate_batch(&mut runtimes, black_box(&context)));
            samples.push(start.elapsed().as_nanos() as u64);
        }
        report_latency_distribution(label, &mut samples);
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
    report_latency_distribution("contexts_8x8x8", &mut samples);

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

fn report_latency_distribution(label: &str, samples: &mut [u64]) {
    samples.sort_unstable();
    let percentile = |percent: usize| samples[(samples.len() * percent).div_ceil(100) - 1];
    println!(
        "mapping_latency {label} samples={} p50_ns={} p95_ns={} p99_ns={}",
        samples.len(),
        percentile(50),
        percentile(95),
        percentile(99),
    );
}

fn evaluation_context<'a>(
    inputs: &'a RuntimeInputSnapshot,
    registries: &'a RuntimeRegistries<'a>,
) -> EvaluationCtx<'a> {
    EvaluationCtx {
        logical_tick: 1,
        delta_time: Duration::from_millis(16),
        events: &[],
        inputs,
        registries,
    }
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

#[derive(Clone, Copy)]
enum Workload {
    NumericChain,
    Sum,
    PackVec3,
    MixedPassthrough,
}

fn build_case(
    processor_count: usize,
    source_count: usize,
    depth: usize,
    workload: Workload,
) -> (Vec<ManagedFormulaRuntime>, RuntimeInputSnapshot, ValueTypeRegistry) {
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
    for processor in 0..processor_count {
        let mut instance = formula.instantiate();
        let sources: Vec<_> = (0..source_count)
            .map(|index| {
                let source = StableRef::new(
                    ValueTypeId::new("chataigne.module_endpoint"),
                    format!("source/{processor}/{index}"),
                );
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
        };
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
            .reconcile_input_source_schema(|reference| {
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
            })
            .expect("benchmark sources should resolve");
        runtimes.push(runtime);
    }
    (runtimes, inputs, value_types)
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
    mapping_runtime_allocation_report
);
criterion_main!(benches);

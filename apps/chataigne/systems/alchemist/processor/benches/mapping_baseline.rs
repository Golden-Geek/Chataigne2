use std::{hint::black_box, time::Duration};

use chataigne_alchemist::{
    ANodeDeclaration, ANodeInstance, EvaluationCtx, ManagedItemId, ManagedItemInstance, ManagedItemUiState,
    PipelineLoweringCtx, PrimitiveNodeDeclaration, PrimitiveNodeKind, RuntimeInputSnapshot, RuntimeRegistries,
    SocketId, ValueTypeId,
};
use chataigne_processor::{
    ValueLaneKey, ValueSet, ValueSetEntry, ValueSetPipelineRuntime,
    alchemist::{node_registry, value_type_registry},
};
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use golden_values::Value as RuntimeValue;

// This is the pre-change managed-runner reference. Keep the same fixture while
// replacing the runner, so changed output retrieval and lane work remain comparable.
fn mapping_baseline(c: &mut Criterion) {
    let value_types = value_type_registry();
    let nodes = node_registry();
    let lowering = PipelineLoweringCtx {
        value_types: &value_types,
        nodes: &nodes,
        properties: None,
    };
    let inputs = RuntimeInputSnapshot::default();
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let context = EvaluationCtx {
        logical_tick: 1,
        delta_time: Duration::from_millis(16),
        events: &[],
        inputs: &inputs,
        registries: &registries,
    };

    let mut group = c.benchmark_group("managed_mapping_float");
    for (channels, depth) in [(1, 1), (8, 1), (32, 1), (8, 8), (32, 8)] {
        let items = (0..depth)
            .map(|index| if index % 2 == 0 { remap() } else { clamp() })
            .collect();
        let mut runtime = ValueSetPipelineRuntime::compile_elementwise(items, ValueTypeId::new("float"), &lowering)
            .expect("baseline filter graph must compile");
        let values = ValueSet::with_entries(
            1,
            (0..channels)
                .map(|index| {
                    ValueSetEntry::new(
                        ValueLaneKey::new(format!("input:{index}")).unwrap(),
                        format!("Input {index}"),
                        RuntimeValue::Float(index as f64 / channels as f64),
                    )
                })
                .collect(),
        );
        let (warmup, output) = runtime.evaluate(&values, &context).unwrap();
        assert_eq!(warmup.entries.len(), channels);
        assert!(output.diagnostics.is_empty());

        group.throughput(Throughput::Elements(channels as u64));
        group.bench_with_input(
            BenchmarkId::new(format!("depth_{depth}"), channels),
            &values,
            |bench, values| {
                bench.iter(|| {
                    let (mapped, output) = runtime.evaluate(black_box(values), &context).unwrap();
                    assert!(output.diagnostics.is_empty());
                    black_box(mapped)
                });
            },
        );
    }
    group.finish();
}

fn item(kind: PrimitiveNodeKind) -> ManagedItemInstance {
    let declaration = PrimitiveNodeDeclaration::new(kind);
    ManagedItemInstance {
        id: ManagedItemId::new(),
        anode: ANodeInstance::new(declaration.type_id(), declaration.label()),
        enabled: true,
        ui_state: ManagedItemUiState::default(),
    }
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

criterion_group!(benches, mapping_baseline);
criterion_main!(benches);

use chataigne_alchemist::{
    CompileCtx, ManagedFilterValueMode, ManagedRegionId, PrimitiveNodeKind, RuntimeInputSnapshot, RuntimeRegistries,
    SocketId,
};
use golden_values::Value as RuntimeValue;

use super::managed_formula::{
    command_target, endpoint_ref, eval_ctx, formula_and_instance, input_item, managed_item_for_primitive, output_item,
    region, registries, remap_item,
};
use crate::{ChannelSourceSchema, ManagedFormulaRuntime};

#[test]
fn tuple_mapping_runs_remap_sum_smooth_and_sends_to_two_commands() {
    let (mut formula, mut instance) = formula_and_instance();
    let filter = formula
        .surface
        .managed_regions
        .iter_mut()
        .find(|definition| definition.id == ManagedRegionId::new("filters"))
        .unwrap();
    filter.filter_value_mode = ManagedFilterValueMode::Tuple;
    let mut second_output = formula
        .surface
        .managed_regions
        .iter()
        .find(|definition| definition.id == ManagedRegionId::new("outputs"))
        .unwrap()
        .clone();
    second_output.id = ManagedRegionId::new("outputs2");
    second_output.label = "Second Output".into();
    formula.surface.managed_regions.push(second_output);

    let sources = [
        endpoint_ref("module/x"),
        endpoint_ref("module/y"),
        endpoint_ref("module/z"),
    ];
    instance.managed_regions.regions.insert(
        ManagedRegionId::new("inputs"),
        region(
            "inputs",
            vec![
                input_item("X", sources[0].clone()),
                input_item("Y", sources[1].clone()),
                input_item("Z", sources[2].clone()),
            ],
        ),
    );
    let mut sum = managed_item_for_primitive(PrimitiveNodeKind::Sum);
    sum.anode.config.set("num_inputs", RuntimeValue::Int(3));
    let mut smooth = managed_item_for_primitive(PrimitiveNodeKind::SmoothFilter);
    smooth.anode.config.set("method", RuntimeValue::String("sma".into()));
    smooth.anode.config.set("window", RuntimeValue::Int(2));
    instance.managed_regions.regions.insert(
        ManagedRegionId::new("filters"),
        region("filters", vec![remap_item(0.0, 10.0, 0.0, 1.0), sum, smooth]),
    );
    let targets = [command_target("target/one"), command_target("target/two")];
    for (region_id, target) in [("outputs", &targets[0]), ("outputs2", &targets[1])] {
        instance.managed_regions.regions.insert(
            ManagedRegionId::new(region_id),
            region(region_id, vec![output_item(region_id, target.clone())]),
        );
    }

    let (value_types, nodes) = registries();
    let compile_ctx = CompileCtx {
        value_types: &value_types,
        nodes: &nodes,
        properties: Some(&formula.properties),
    };
    let mut runtime = ManagedFormulaRuntime::compile(&formula, &instance, &compile_ctx)
        .unwrap()
        .unwrap();
    runtime
        .reconcile_input_source_schema(|_| {
            Some(ChannelSourceSchema {
                value_type: "float".into(),
                metadata: Default::default(),
            })
        })
        .unwrap();
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    for (tick, x, expected) in [(1, 2.0, 1.2), (2, 4.0, 1.3)] {
        let mut inputs = RuntimeInputSnapshot::default();
        for (source, value) in sources.iter().zip([x, 4.0, 6.0]) {
            inputs.insert(source.clone(), RuntimeValue::Float(value));
        }
        let output = runtime.evaluate(&eval_ctx(tick, &inputs, &registries));
        assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
        assert_eq!(output.intents.len(), 2);
        for (intent, target) in output.intents.iter().zip(&targets) {
            assert_eq!(intent.target.as_ref(), Some(target));
            assert!(matches!(intent.payload, RuntimeValue::Float(value) if (value - expected).abs() < 1e-12));
        }
    }
}

#[test]
fn tuple_mapping_sends_xyz_as_one_vec3_command_value() {
    let (mut formula, mut instance) = formula_and_instance();
    formula
        .surface
        .managed_regions
        .iter_mut()
        .find(|definition| definition.id == ManagedRegionId::new("filters"))
        .unwrap()
        .filter_value_mode = ManagedFilterValueMode::Tuple;
    let sources = [
        endpoint_ref("module/x"),
        endpoint_ref("module/y"),
        endpoint_ref("module/z"),
    ];
    instance.managed_regions.regions.insert(
        ManagedRegionId::new("inputs"),
        region(
            "inputs",
            vec![
                input_item("X", sources[0].clone()),
                input_item("Y", sources[1].clone()),
                input_item("Z", sources[2].clone()),
            ],
        ),
    );
    let mut math = managed_item_for_primitive(PrimitiveNodeKind::Math);
    math.anode
        .config
        .set("application", RuntimeValue::String("each".into()));
    math.anode
        .input_defaults
        .insert(SocketId::new("value2"), RuntimeValue::Float(1.0));
    instance.managed_regions.regions.insert(
        ManagedRegionId::new("filters"),
        region(
            "filters",
            vec![math, managed_item_for_primitive(PrimitiveNodeKind::PackVec3)],
        ),
    );
    let target = command_target("target/xyz");
    instance.managed_regions.regions.insert(
        ManagedRegionId::new("outputs"),
        region("outputs", vec![output_item("XYZ", target.clone())]),
    );

    let (value_types, nodes) = registries();
    let compile_ctx = CompileCtx {
        value_types: &value_types,
        nodes: &nodes,
        properties: Some(&formula.properties),
    };
    let mut runtime = ManagedFormulaRuntime::compile(&formula, &instance, &compile_ctx)
        .unwrap()
        .unwrap();
    runtime
        .reconcile_input_source_schema(|_| {
            Some(ChannelSourceSchema {
                value_type: "float".into(),
                metadata: Default::default(),
            })
        })
        .unwrap();
    let mut inputs = RuntimeInputSnapshot::default();
    for (source, value) in sources.iter().zip([1.0, 2.0, 3.0]) {
        inputs.insert(source.clone(), RuntimeValue::Float(value));
    }
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let output = runtime.evaluate(&eval_ctx(1, &inputs, &registries));
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    assert_eq!(output.intents.len(), 1);
    assert_eq!(output.intents[0].target.as_ref(), Some(&target));
    assert_eq!(output.intents[0].payload, RuntimeValue::Vec3([2.0, 3.0, 4.0]));
}

#[test]
fn tuple_mapping_reports_mixed_input_instead_of_filtering_one_source() {
    let (mut formula, mut instance) = formula_and_instance();
    formula
        .surface
        .managed_regions
        .iter_mut()
        .find(|definition| definition.id == ManagedRegionId::new("filters"))
        .unwrap()
        .filter_value_mode = ManagedFilterValueMode::Tuple;
    let sources = [endpoint_ref("module/x"), endpoint_ref("module/flag")];
    instance.managed_regions.regions.insert(
        ManagedRegionId::new("inputs"),
        region(
            "inputs",
            vec![
                input_item("X", sources[0].clone()),
                input_item("Flag", sources[1].clone()),
            ],
        ),
    );
    instance.managed_regions.regions.insert(
        ManagedRegionId::new("filters"),
        region("filters", vec![remap_item(0.0, 1.0, 0.0, 1.0)]),
    );
    instance.managed_regions.regions.insert(
        ManagedRegionId::new("outputs"),
        region("outputs", vec![output_item("Out", command_target("target/out"))]),
    );
    let (value_types, nodes) = registries();
    let compile_ctx = CompileCtx {
        value_types: &value_types,
        nodes: &nodes,
        properties: Some(&formula.properties),
    };
    let mut runtime = ManagedFormulaRuntime::compile(&formula, &instance, &compile_ctx)
        .unwrap()
        .unwrap();
    let error = runtime
        .reconcile_input_source_schema(|source| {
            Some(ChannelSourceSchema {
                value_type: if source == &sources[0] { "float" } else { "bool" }.into(),
                metadata: Default::default(),
            })
        })
        .unwrap_err()
        .into_diagnostic();
    assert_eq!(error.code, "managed_formula_stage_error");
    assert!(error.message.contains("ordered input tuple"), "{}", error.message);
}

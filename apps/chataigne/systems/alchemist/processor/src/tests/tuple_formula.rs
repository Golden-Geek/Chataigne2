use chataigne_alchemist::{
    ColorValue, CompileCtx, ManagedFilterValueMode, ManagedRegionId, PrimitiveNodeKind, RuntimeInputSnapshot,
    RuntimeRegistries, SocketId, ValueComponent,
};
use golden_values::Value as RuntimeValue;

use super::managed_formula::{
    command_target, endpoint_ref, eval_ctx, formula_and_instance, input_item, managed_item_for_primitive, output_item,
    region, registries, remap_item,
};
use crate::{
    ChannelSourceSchema, CommandArgumentValues, ManagedFormulaRuntime, OUTPUT_BINDINGS_FIELD, OutputArgumentBinding,
    OutputBindingConfig, OutputValueSource, ValueLaneKey,
};

#[test]
fn tuple_mapping_extracts_color_alpha_for_a_typed_command() {
    let (mut formula, mut instance) = formula_and_instance();
    formula
        .surface
        .managed_regions
        .iter_mut()
        .find(|region| region.id == ManagedRegionId::new("filters"))
        .unwrap()
        .filter_value_mode = ManagedFilterValueMode::Tuple;
    let source = endpoint_ref("module/color");
    let input = input_item("Color", source.clone());
    let alpha_key = ValueLaneKey::input(input.id).extracted(ValueComponent::A);
    instance
        .managed_regions
        .regions
        .insert(ManagedRegionId::new("inputs"), region("inputs", vec![input]));
    instance.managed_regions.regions.insert(
        ManagedRegionId::new("filters"),
        region(
            "filters",
            vec![managed_item_for_primitive(PrimitiveNodeKind::ExtractColor)],
        ),
    );
    let target = command_target("target/alpha");
    let argument = command_target("target/alpha/value");
    let mut output = output_item("Alpha", target.clone());
    output.anode.config.set(
        OUTPUT_BINDINGS_FIELD,
        OutputBindingConfig {
            value: OutputValueSource::Element(alpha_key.clone()),
            arguments: vec![OutputArgumentBinding {
                parameter: argument.clone(),
                source: OutputValueSource::Element(alpha_key),
            }],
            ..OutputBindingConfig::default()
        }
        .to_runtime_value()
        .unwrap(),
    );
    instance
        .managed_regions
        .regions
        .insert(ManagedRegionId::new("outputs"), region("outputs", vec![output]));

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
                value_type: "color".into(),
                metadata: Default::default(),
            })
        })
        .unwrap();
    let mut inputs = RuntimeInputSnapshot::default();
    inputs.insert(
        source,
        RuntimeValue::Color(ColorValue {
            red: 0.1,
            green: 0.2,
            blue: 0.3,
            alpha: 0.4,
        }),
    );
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let evaluated = runtime.evaluate(&eval_ctx(1, &inputs, &registries));
    assert!(evaluated.diagnostics.is_empty(), "{:?}", evaluated.diagnostics);
    assert_eq!(evaluated.intents.len(), 1);
    assert_eq!(evaluated.intents[0].target.as_ref(), Some(&target));
    let payload = CommandArgumentValues::from_runtime_value(&evaluated.intents[0].payload)
        .unwrap()
        .unwrap();
    assert_eq!(payload.value, RuntimeValue::Float(0.4));
    assert_eq!(payload.arguments[0].parameter, argument);
    assert_eq!(payload.arguments[0].value, RuntimeValue::Float(0.4));
}

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
    let arguments = [command_target("target/one/value"), command_target("target/two/value")];
    let outputs = targets
        .iter()
        .zip(&arguments)
        .map(|(target, argument)| {
            let mut output = output_item("Command", target.clone());
            output.anode.config.set(
                OUTPUT_BINDINGS_FIELD,
                OutputBindingConfig {
                    arguments: vec![OutputArgumentBinding {
                        parameter: argument.clone(),
                        source: OutputValueSource::Whole,
                    }],
                    ..OutputBindingConfig::default()
                }
                .to_runtime_value()
                .unwrap(),
            );
            output
        })
        .collect();
    instance
        .managed_regions
        .regions
        .insert(ManagedRegionId::new("outputs"), region("outputs", outputs));

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
        for ((intent, target), argument) in output.intents.iter().zip(&targets).zip(&arguments) {
            assert_eq!(intent.target.as_ref(), Some(target));
            let payload = CommandArgumentValues::from_runtime_value(&intent.payload)
                .unwrap()
                .unwrap();
            assert!(matches!(payload.value, RuntimeValue::Float(value) if (value - expected).abs() < 1e-12));
            assert_eq!(payload.arguments[0].parameter, *argument);
            assert!(
                matches!(payload.arguments[0].value, RuntimeValue::Float(value) if (value - expected).abs() < 1e-12)
            );
        }
    }
}

#[test]
fn tuple_mapping_binds_packed_xyz_to_one_vec3_command_argument() {
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
    let argument = command_target("target/xyz/position");
    let mut output = output_item("XYZ", target.clone());
    output.anode.config.set(
        OUTPUT_BINDINGS_FIELD,
        OutputBindingConfig {
            arguments: vec![OutputArgumentBinding {
                parameter: argument.clone(),
                source: OutputValueSource::Whole,
            }],
            ..OutputBindingConfig::default()
        }
        .to_runtime_value()
        .unwrap(),
    );
    instance
        .managed_regions
        .regions
        .insert(ManagedRegionId::new("outputs"), region("outputs", vec![output]));

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
    let payload = CommandArgumentValues::from_runtime_value(&output.intents[0].payload)
        .unwrap()
        .unwrap();
    assert_eq!(payload.value, RuntimeValue::Vec3([2.0, 3.0, 4.0]));
    assert_eq!(payload.arguments[0].parameter, argument);
    assert_eq!(payload.arguments[0].value, RuntimeValue::Vec3([2.0, 3.0, 4.0]));
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

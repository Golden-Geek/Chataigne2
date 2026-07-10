use std::{collections::BTreeMap, sync::Arc};

use golden_alchemist::{ANodeRegistry, EvaluationOptions, FormulaCompileCache};
use golden_condition::{ConditionCompiler, ConditionExpr, ConditionInputId};
use golden_context::{ContextAxis, ContextItem, ContextLayer, ContextLayerMode, ContextLimits};
use golden_model::EntityId;
use golden_values::{FiniteF64, Value};

use super::*;

#[test]
fn mapping_creation_uses_one_path_and_processor_lanes_share_a_kernel() {
    let formula = BuiltinFormulaAssets::mapping();
    let condition_input = formula.surface.inputs[0].id;
    let value_input = formula.surface.inputs[1].id;
    let output = formula.surface.outputs[0].id;
    let enabled = ConditionInputId("enabled".into());
    let condition = Arc::new(
        ConditionCompiler
            .compile(&ConditionExpr::Truthy(enabled.clone()))
            .unwrap(),
    );
    let definition = ProcessorDefinition::mapping(
        ProcessorId(EntityId::new()),
        MappingSpec {
            formula: formula.id,
            inputs: vec![
                ProcessorSurfaceBinding {
                    source: "condition".into(),
                    target: condition_input,
                },
                ProcessorSurfaceBinding {
                    source: "value".into(),
                    target: value_input,
                },
            ],
            outputs: vec![output],
            condition: Some(condition),
        },
    )
    .unwrap();
    let axis = ContextAxis {
        id: EntityId::new(),
        label: "fixtures".into(),
        items: vec![
            ContextItem {
                id: EntityId::new(),
                label: "a".into(),
                value: Value::Integer(0),
            },
            ContextItem {
                id: EntityId::new(),
                label: "b".into(),
                value: Value::Integer(1),
            },
        ],
    };
    let cache = FormulaCompileCache::default();
    let registry = ANodeRegistry::with_builtins();
    let plan = ProcessorCompiler
        .compile(
            definition,
            &formula,
            &[ContextLayer {
                mode: ContextLayerMode::Accumulate,
                axes: vec![axis],
            }],
            ContextLimits::default(),
            &cache,
            &registry,
        )
        .unwrap();
    let kernel = Arc::clone(&plan.kernel);
    let mut runtime = ProcessorRuntime::new(plan);
    assert!(Arc::ptr_eq(&kernel, runtime.kernel()));
    assert_eq!(runtime.lane_keys().len(), 2);
    for lane in 0..2 {
        runtime.set_input(lane, condition_input, Value::Bool(true)).unwrap();
        runtime
            .set_input(
                lane,
                value_input,
                Value::Float(FiniteF64::new(lane as f64 + 1.0).unwrap()),
            )
            .unwrap();
    }
    let condition_inputs = vec![
        BTreeMap::from([(enabled.clone(), Value::Bool(true))]),
        BTreeMap::from([(enabled, Value::Bool(false))]),
    ];
    let report = runtime
        .evaluate(&condition_inputs, EvaluationOptions::default())
        .unwrap();
    assert_eq!(report.condition_rejected, 1);
    assert_eq!(report.executed_operations, 1);
    assert_eq!(
        runtime.output(0, output).unwrap(),
        &Value::Float(FiniteF64::new(1.0).unwrap())
    );
    assert_eq!(cache.compilation_count(), 1);
}

#[test]
fn action_and_mapping_are_real_compilable_formula_assets() {
    let registry = ANodeRegistry::with_builtins();
    let action = BuiltinFormulaAssets::action();
    let mapping = BuiltinFormulaAssets::mapping();
    assert!(
        golden_alchemist::FormulaCompiler::new(&registry)
            .compile(&action)
            .is_ok()
    );
    assert!(
        golden_alchemist::FormulaCompiler::new(&registry)
            .compile(&mapping)
            .is_ok()
    );
    assert_eq!(action.id, ACTION_FORMULA_ID);
    assert_eq!(mapping.id, MAPPING_FORMULA_ID);
}

#[test]
fn single_multi_input_and_conditioned_mappings_share_one_creation_path() {
    let formula = BuiltinFormulaAssets::mapping();
    let input = formula.surface.inputs[0].id;
    let output = formula.surface.outputs[0].id;
    for input_count in [1, 3] {
        let inputs = (0..input_count)
            .map(|index| ProcessorSurfaceBinding {
                source: format!("input-{index}").into(),
                target: input,
            })
            .collect();
        let definition = ProcessorDefinition::mapping(
            ProcessorId(EntityId::new()),
            MappingSpec {
                formula: formula.id,
                inputs,
                outputs: vec![output],
                condition: (input_count > 1)
                    .then(|| Arc::new(ConditionCompiler.compile(&ConditionExpr::Literal(true)).unwrap())),
            },
        )
        .unwrap();
        assert_eq!(definition.inputs.len(), input_count);
    }
}

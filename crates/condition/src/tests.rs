use std::{collections::HashMap, sync::Arc, time::Duration};

use golden_values::{StableRef, TriggerValue, Value, ValueTypeId};

use crate::{
    ConditionBehavior, ConditionDefinition, ConditionEvaluationFrame, ConditionGroupPolicy, ConditionInputProvider,
    ConditionProjection, ConditionRuntime, EdgePolicy, TypedComparator, compile_condition,
};

#[derive(Default)]
struct Inputs(HashMap<StableRef, Value>);

impl ConditionInputProvider for Inputs {
    fn input_value(&self, input: &StableRef) -> Option<Value> {
        self.0.get(input).cloned()
    }
}

fn input(name: &str) -> StableRef {
    StableRef::new(ValueTypeId::new("number"), name)
}

fn evaluate(
    runtime: &mut ConditionRuntime,
    program: &crate::CompiledConditionProgram,
    inputs: &Inputs,
    tick: u64,
) -> bool {
    runtime
        .evaluate(
            program,
            &ConditionEvaluationFrame {
                logical_tick: tick,
                delta_time: Duration::from_millis(100),
                inputs,
            },
        )
        .unwrap()
        .value
}

#[test]
fn compiled_groups_evaluate_without_authoring_tree_access() {
    let left_ref = input("left");
    let right_ref = input("right");
    let mut left = ConditionDefinition::input_value("left", left_ref.clone(), Value::Float(2.0));
    let mut right = ConditionDefinition::input_value("right", right_ref.clone(), Value::Bool(true));
    if let crate::ConditionKind::InputValue(condition) = &mut left.kind {
        condition.comparator = TypedComparator::Greater;
    }
    if let crate::ConditionKind::InputValue(condition) = &mut right.kind {
        condition.comparator = TypedComparator::Equal;
    }
    let definition = ConditionDefinition::group("root", ConditionGroupPolicy::All, vec![left, right]);
    let program = compile_condition(&definition).unwrap();
    let mut runtime = ConditionRuntime::new(&program);
    let inputs = Inputs(HashMap::from([
        (left_ref, Value::Float(3.0)),
        (right_ref, Value::Bool(true)),
    ]));

    assert!(evaluate(&mut runtime, &program, &inputs, 1));
    assert_eq!(program.instructions.len(), 3);
    assert_eq!(program.state_layout.keys.len(), 2);
}

#[test]
fn vector_projection_speed_toggle_and_transient_state_are_dense_and_migratable() {
    let source = input("source");
    let mut definition = ConditionDefinition::input_value("speed", source.clone(), Value::Float(5.0));
    if let crate::ConditionKind::InputValue(condition) = &mut definition.kind {
        condition.projection = ConditionProjection::Speed;
        condition.comparator = TypedComparator::Greater;
        condition.behavior = ConditionBehavior {
            edge: EdgePolicy::Rising,
            toggle: true,
            transient_ticks: 2,
            ..ConditionBehavior::default()
        };
    }
    let program = compile_condition(&definition).unwrap();
    let mut runtime = ConditionRuntime::new(&program);
    let mut inputs = Inputs(HashMap::from([(source.clone(), Value::Float(1.0))]));
    assert!(!evaluate(&mut runtime, &program, &inputs, 1));
    inputs.0.insert(source.clone(), Value::Float(2.0));
    assert!(evaluate(&mut runtime, &program, &inputs, 2));
    inputs.0.insert(source, Value::Float(2.0));
    assert!(evaluate(&mut runtime, &program, &inputs, 3));

    let migrated = runtime.migrate(&program);
    assert_eq!(migrated.clone().migrate(&program).state_keys(), migrated.state_keys());
}

#[test]
fn changed_and_trigger_comparators_keep_previous_value_state() {
    let source = input("trigger");
    let mut definition = ConditionDefinition::input_value("trigger", source.clone(), Value::Unit);
    if let crate::ConditionKind::InputValue(condition) = &mut definition.kind {
        condition.comparator = TypedComparator::Triggered;
    }
    let program = compile_condition(&definition).unwrap();
    let mut runtime = ConditionRuntime::new(&program);
    let inputs = Inputs(HashMap::from([(
        source,
        Value::Trigger(TriggerValue {
            fired: true,
            edge_id: 7,
            logical_tick: 1,
        }),
    )]));
    assert!(evaluate(&mut runtime, &program, &inputs, 1));

    let string_ref = input("string");
    let mut changed =
        ConditionDefinition::input_value("changed", string_ref.clone(), Value::String(Arc::from("ignored")));
    if let crate::ConditionKind::InputValue(condition) = &mut changed.kind {
        condition.comparator = TypedComparator::Changed;
    }
    let changed_program = compile_condition(&changed).unwrap();
    let mut changed_runtime = ConditionRuntime::new(&changed_program);
    let mut values = Inputs(HashMap::from([(string_ref.clone(), Value::String(Arc::from("a")))]));
    assert!(!evaluate(&mut changed_runtime, &changed_program, &values, 1));
    values.0.insert(string_ref, Value::String(Arc::from("b")));
    assert!(evaluate(&mut changed_runtime, &changed_program, &values, 2));
}

#[test]
fn empty_groups_are_compile_errors() {
    let definition = ConditionDefinition::group("empty", ConditionGroupPolicy::Any, Vec::new());
    let diagnostics = compile_condition(&definition).unwrap_err();
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("no children"));
}

#[test]
fn compiled_shadow_semantics_match_the_reference_comparators_without_effects() {
    let cases = [
        (Value::Float(4.0), Value::Float(3.0), TypedComparator::Greater, true),
        (Value::Float(2.0), Value::Float(3.0), TypedComparator::Less, true),
        (Value::Bool(true), Value::Bool(true), TypedComparator::Equal, true),
        (Value::Bool(false), Value::Bool(true), TypedComparator::NotEqual, true),
    ];

    for (index, (actual, expected, comparator, legacy_result)) in cases.into_iter().enumerate() {
        let source = input(&format!("shadow-{index}"));
        let mut definition = ConditionDefinition::input_value("shadow", source.clone(), expected);
        if let crate::ConditionKind::InputValue(condition) = &mut definition.kind {
            condition.comparator = comparator;
        }
        let program = compile_condition(&definition).unwrap();
        let mut runtime = ConditionRuntime::new(&program);
        let inputs = Inputs(HashMap::from([(source, actual)]));

        assert_eq!(evaluate(&mut runtime, &program, &inputs, 1), legacy_result);
    }
}

use std::collections::BTreeMap;

use golden_values::Value;

use super::*;

#[test]
fn nested_conditions_compile_to_postfix_ir_and_evaluate_without_authored_walks() {
    let enabled = ConditionInputId("enabled".into());
    let count = ConditionInputId("count".into());
    let expression = ConditionExpr::All(vec![
        ConditionExpr::Truthy(enabled.clone()),
        ConditionExpr::Compare {
            input: count.clone(),
            comparison: Comparison::GreaterOrEqual,
            expected: Value::Integer(2),
        },
    ]);
    let program = ConditionCompiler.compile(&expression).unwrap();
    assert_eq!(program.operations().len(), 3);

    let inputs = BTreeMap::from([(enabled, Value::Bool(true)), (count, Value::Integer(3))]);
    assert!(program.evaluate(&inputs).unwrap());
}

#[test]
fn empty_boolean_groups_have_explicit_identities() {
    let all = ConditionCompiler.compile(&ConditionExpr::All(Vec::new())).unwrap();
    let any = ConditionCompiler.compile(&ConditionExpr::Any(Vec::new())).unwrap();
    assert!(all.evaluate(&BTreeMap::new()).unwrap());
    assert!(!any.evaluate(&BTreeMap::new()).unwrap());
}

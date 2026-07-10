use super::*;

fn float(value: f64) -> Value {
    Value::Float(FiniteF64::new(value).unwrap())
}

#[test]
fn declarations_reject_type_and_constraint_mismatches() {
    let declaration = ParameterDeclaration {
        id: EntityId::new(),
        value_type: "float".into(),
        default: float(0.5),
        constraints: vec![Constraint::Numeric {
            minimum: Some(FiniteF64::new(0.0).unwrap()),
            maximum: Some(FiniteF64::new(1.0).unwrap()),
        }],
        ui_hints: BTreeMap::new(),
    };

    assert_eq!(
        declaration.validate(&float(2.0)),
        Err(ParameterError::ConstraintViolation)
    );
    assert!(matches!(
        declaration.validate(&Value::Bool(false)),
        Err(ParameterError::TypeMismatch { .. })
    ));
}

#[test]
fn unchanged_values_do_not_advance_if_changed_state() {
    let mut state = ParameterState {
        declaration: EntityId::new(),
        control: ControlMode::Static,
        value: Value::Bool(true),
        revision: Revision::new(3),
    };
    assert!(!state.apply(Value::Bool(true), ChangeBehavior::IfChanged).unwrap());
    assert_eq!(state.revision, Revision::new(3));
    assert!(state.apply(Value::Bool(false), ChangeBehavior::IfChanged).unwrap());
    assert_eq!(state.revision, Revision::new(4));
}

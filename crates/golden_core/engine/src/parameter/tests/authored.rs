use super::*;
use crate::{
    node::{NodeAuthoredData, NodeId, NodeReference, NodeUuid},
    parameter::{
        ParameterControlDiagnostic, ParameterControlMode, ParameterControlSpec, ParameterUiHints, RangeConstraint,
    },
};

fn option(id: &str, value: ParamValue) -> ParameterEnumOption {
    ParameterEnumOption {
        variant_id: id.into(),
        value,
        label: id.into(),
        tags: vec![],
        ordering: None,
    }
}

#[test]
fn authored_checkpoint_keeps_large_native_domain_admission_through_raw_restore() {
    let domain = ParameterEnumDomain::new(
        (0..32_768)
            .map(|index| {
                let id = format!("variant-{index}");
                option(&id, ParamValue::Enum(id.clone()))
            })
            .collect(),
    );
    let weight = domain.retained_bytes();
    let pool = golden_io::PayloadPool::new("parameter-authored-domain", 1, 64 * 1024 * 1024).unwrap();
    let shared = pool
        .try_reserve(weight)
        .unwrap()
        .share_retained(domain, weight)
        .unwrap();
    let options = ParameterEnumOptions::from_shared(shared.arc().clone());
    let canonical = options.shared().clone();
    let mut parameter = Parameter::new(
        "Device",
        ParamValue::Enum("variant-0".into()),
        ParameterChangeCheck::ValueChange,
    );
    parameter.constraints.enum_options = options;
    let captured = parameter.engine_capture_authored_data().unwrap();
    assert!(
        captured.as_json().is_err(),
        "history must not materialize an option-array JSON tree"
    );
    let started = std::time::Instant::now();
    for _ in 0..128 {
        parameter.constraints.enum_options = Default::default();
        parameter.value = ParamValue::Enum("other".into());
        parameter.engine_restore_authored_data(captured.clone()).unwrap();
        assert!(std::sync::Arc::ptr_eq(
            parameter.constraints.enum_options.shared(),
            &canonical
        ));
        assert!(parameter.constraints.enum_options.contains_variant("variant-32767"));
        assert_eq!(parameter.value, ParamValue::Enum("variant-0".into()));
    }
    eprintln!(
        "128 native restores of one 32768-entry shared domain: {:?}",
        started.elapsed()
    );
    drop((parameter, shared, canonical));
    assert!(
        pool.try_reserve(1).is_err(),
        "the checkpoint alone retains native admission"
    );
    drop(captured);
    assert!(pool.wait_for_idle(std::time::Duration::from_secs(3)));
}

#[test]
fn native_parameter_checkpoint_restores_all_authored_fields_and_rejects_wrong_payloads() {
    let mut parameter = Parameter::new("Value", ParamValue::Float(1.0), ParameterChangeCheck::None);
    parameter.default_value = ParamValue::Float(2.0);
    parameter.event_behaviour = ParameterEventBehaviour::Append;
    parameter.read_only = true;
    parameter.persist_read_only_value = true;
    parameter.constraints.range = RangeConstraint::uniform(Some(-1.0), Some(2.0));
    parameter.constraints.step = Some(0.5);
    parameter.constraints.step_base = Some(0.25);
    parameter.ui_hints = ParameterUiHints {
        widget: Some("knob".into()),
        unit: Some("dB".into()),
    };
    parameter.control.mode = ParameterControlMode::Expression;
    parameter.control.spec = ParameterControlSpec::Expression;
    parameter.control.diagnostics = vec![ParameterControlDiagnostic::new("example", "detail")];
    parameter.control_modes_enabled = false;
    let expected = parameter.project_encode_data().unwrap();
    let captured = parameter.engine_capture_authored_data().unwrap();
    parameter.value = ParamValue::Str("wrong kind".into());
    parameter.default_value = ParamValue::Str("wrong default".into());
    parameter.change_check = ParameterChangeCheck::ValueChange;
    parameter.event_behaviour = ParameterEventBehaviour::Coalesce;
    parameter.read_only = false;
    parameter.persist_read_only_value = false;
    parameter.constraints = Default::default();
    parameter.ui_hints = Default::default();
    parameter.control = Default::default();
    parameter.control_modes_enabled = true;
    parameter.engine_restore_authored_data(captured).unwrap();
    assert_eq!(parameter.project_encode_data().unwrap(), expected);
    assert!(parameter.persist_read_only_value);
    let before = parameter.snapshot();
    assert!(
        parameter
            .engine_restore_authored_data(NodeAuthoredData::json(serde_json::Value::Null))
            .is_err()
    );
    assert_eq!(parameter.snapshot(), before);
}

#[test]
fn authored_reference_equality_includes_persisted_hints_and_excludes_runtime_ids() {
    let mut parameter = Parameter::new(
        "Reference",
        ParamValue::Reference(NodeReference::new(NodeUuid::nil())),
        ParameterChangeCheck::ValueChange,
    );
    let captured = parameter.engine_capture_authored_data().unwrap();
    let ParamValue::Reference(reference) = &mut parameter.value else {
        unreachable!()
    };
    reference.cached_id = Some(NodeId(42));
    assert_eq!(parameter.engine_capture_authored_data().unwrap(), captured);
    let ParamValue::Reference(reference) = &mut parameter.value else {
        unreachable!()
    };
    reference.cached_name = Some("persisted display".into());
    assert_ne!(parameter.engine_capture_authored_data().unwrap(), captured);
    parameter.engine_restore_authored_data(captured.clone()).unwrap();
    let ParamValue::Reference(reference) = &mut parameter.value else {
        unreachable!()
    };
    reference.relative_path_from_root = vec!["persisted".into(), "path".into()];
    assert_ne!(parameter.engine_capture_authored_data().unwrap(), captured);
}

#[test]
fn authored_enum_reference_hint_changes_are_distinct_from_runtime_constraint_equality() {
    let reference = NodeReference::new(NodeUuid::nil());
    let mut parameter = Parameter::new(
        "Reference option",
        ParamValue::Enum("choice".into()),
        ParameterChangeCheck::ValueChange,
    );
    parameter.constraints.enum_options = vec![option("choice", ParamValue::Reference(reference.clone()))].into();
    let before = parameter.constraints.clone();
    let captured = parameter.engine_capture_authored_data().unwrap();
    let mut runtime = reference.clone();
    runtime.cached_id = Some(NodeId(42));
    parameter.constraints.enum_options = vec![option("choice", ParamValue::Reference(runtime))].into();
    assert_eq!(parameter.engine_capture_authored_data().unwrap(), captured);
    let mut authored = reference;
    authored.cached_name = Some("new persisted hint".into());
    parameter.constraints.enum_options = vec![option("choice", ParamValue::Reference(authored))].into();
    assert_eq!(
        parameter.constraints, before,
        "ordinary constraints compare semantic reference identity"
    );
    assert_ne!(
        parameter.engine_capture_authored_data().unwrap(),
        captured,
        "authored checkpoints also compare persisted hints"
    );
}

#[test]
fn native_checkpoints_preserve_float_bits_without_relaxing_value_validation() {
    let nan = f64::from_bits(0x7ff8_0000_0000_0042);
    let mut parameter = Parameter::new(
        "Native value",
        ParamValue::Float(nan),
        ParameterChangeCheck::ValueChange,
    );
    parameter.constraints.step = Some(nan);
    parameter.constraints.range = Some(RangeConstraint::Components {
        min: Some(vec![nan, -0.0]),
        max: None,
    });
    let captured = parameter.engine_capture_authored_data().unwrap();
    assert_eq!(parameter.engine_capture_authored_data().unwrap(), captured);
    parameter.value = ParamValue::Float(0.0);
    parameter.constraints = Default::default();
    parameter.engine_restore_authored_data(captured).unwrap();
    let ParamValue::Float(restored) = parameter.value else {
        panic!("float kind must survive an in-memory restore")
    };
    assert_eq!(restored.to_bits(), nan.to_bits());
    assert_eq!(parameter.constraints.step.unwrap().to_bits(), nan.to_bits());
    assert!(parameter.engine_prepare_param_value(ParamValue::Float(nan)).is_err());
}

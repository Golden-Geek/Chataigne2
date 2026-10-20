use super::*;

#[test]
fn values_and_all_destinations_use_the_captured_snapshot() {
    let mut fixture = Fixture::new();
    let (_, host) = fixture.output(fixture.outputs, "original.example", 8000);
    let request = fixture.request();
    fixture.nodes.get_mut(&host).unwrap().param_value = Some(ParamValue::Str("later.example".into()));
    fixture.nodes.get_mut(&fixture.value).unwrap().param_value = Some(ParamValue::Float(0.75));
    let prepared = prepare(request).unwrap();
    assert_eq!(prepared.targets[0].host, "original.example");
    let message = decode(&prepared.bytes);
    assert_eq!(message.addr, "/gain");
    assert_eq!(message.args, [rosc::OscType::Float(0.25)]);
}

#[test]
fn large_output_tree_yields_before_completing_preparation() {
    let mut fixture = Fixture::new();
    for _ in 0..100 {
        fixture.output(fixture.outputs, "127.0.0.1", 8000);
    }
    let mut preparation = Preparation::new(fixture.request()).unwrap();
    assert!(!preparation.service().unwrap());
    assert_eq!(preparation.edges, PREPARATION_TURN);
    assert!(preparation.targets.len() < 100);
    while !preparation.service().unwrap() {}
    assert_eq!(preparation.finish().unwrap().targets.len(), 100);
}

#[test]
fn disabled_output_subtree_is_not_included_in_the_plan() {
    let mut fixture = Fixture::new();
    fixture.output(fixture.outputs, "included.example", 8000);
    let folder = fixture.add(Some(fixture.outputs), "disabled", "folder", None);
    fixture.nodes.get_mut(&folder).unwrap().enabled = false;
    fixture.output(folder, "excluded.example", 8000);
    let prepared = prepare(fixture.request()).unwrap();
    assert_eq!(prepared.targets.len(), 1);
    assert_eq!(prepared.targets[0].host, "included.example");
}

#[test]
fn malformed_destination_cycle_hits_the_total_work_budget() {
    let mut fixture = Fixture::new();
    let folder = fixture.add(Some(fixture.outputs), "cycle", "folder", None);
    fixture.nodes.get_mut(&folder).unwrap().first_child = Some(folder);
    let error = prepare(fixture.request()).err().unwrap();
    assert!(error.contains("16384 traversal edges"), "{error}");
}

#[test]
fn nested_feedback_has_bounded_address_work_and_numeric_argument_order() {
    let mut fixture = Fixture::new();
    let folder = fixture.add(Some(fixture.values), "tuple", "folder", None);
    fixture.add(Some(folder), "value 10", "parameter", Some(ParamValue::Int(10)));
    let first = fixture.add(Some(folder), "value 1", "parameter", Some(ParamValue::Int(1)));
    fixture.add(Some(folder), "unrelated", "parameter", Some(ParamValue::Int(99)));
    let snapshot = fixture.snapshot();
    let (address, payload) = value_packet(&snapshot, fixture.values, first).unwrap();
    assert_eq!(address, "/tuple");
    assert_eq!(
        payload,
        OscValuePayload::Multi(vec![ParamValue::Int(1), ParamValue::Int(10)])
    );

    let mut parent = fixture.values;
    for _ in 0..65 {
        parent = fixture.add(Some(parent), "level", "folder", None);
    }
    assert!(value_packet(&fixture.snapshot(), fixture.values, parent)
        .unwrap_err()
        .contains("64 levels"));
}

#[test]
fn indexed_folder_target_is_not_promoted_twice_after_tuple_coalescing() {
    let mut fixture = Fixture::new();
    let folder = fixture.add(Some(fixture.values), "value 1", "folder", None);
    let first = fixture.add(Some(folder), "value 1", "parameter", Some(ParamValue::Int(1)));
    assert_eq!(feedback_target(&fixture.snapshot(), first).unwrap(), folder);
    let (address, payload) = value_packet(&fixture.snapshot(), fixture.values, folder).unwrap();
    assert_eq!(address, "/value 1");
    assert_eq!(payload, OscValuePayload::Multi(vec![ParamValue::Int(1)]));
}

#[test]
fn script_capture_bounds_address_arguments_and_retained_storage() {
    let fixture = Fixture::new();
    for address in [
        "no-slash".to_string(),
        "/bad\0address".into(),
        format!("/{}", "x".repeat(4096)),
    ] {
        assert!(matches!(
            fixture.script(vec![ParamValue::Str(address)]),
            Err(QueueError::Invalid(_))
        ));
    }
    let mut args = vec![ParamValue::Str("/count".into())];
    args.extend((0..1025).map(ParamValue::Int));
    assert!(matches!(fixture.script(args), Err(QueueError::Invalid(_))));
    assert!(matches!(
        fixture.script(vec![
            ParamValue::Str("/bytes".into()),
            ParamValue::Str("x".repeat(128 * 1024))
        ]),
        Err(QueueError::Invalid(_))
    ));
}

#[test]
fn captured_command_override_is_decoded_on_the_worker_view() {
    use crate::app::module_command::{
        ModuleCommandDeliveryPolicy, ModuleCommandExecuteEvent, ModuleCommandParamOverride,
    };
    let mut fixture = Fixture::new();
    fixture.output(fixture.outputs, "127.0.0.1", 8000);
    let command = fixture.add(Some(fixture.module), "command", "osc_send_custom_message_command", None);
    let address = fixture.add(
        Some(command),
        "address",
        "parameter",
        Some(ParamValue::Str("/authored".into())),
    );
    let arguments = fixture.add(Some(command), "arguments", "folder", None);
    let argument = fixture.add(Some(arguments), "argument", "parameter", Some(ParamValue::Int(7)));
    let event = golden_core::events::CustomEvent::new(
        "execute",
        Some(command),
        serde_json::to_value(ModuleCommandExecuteEvent {
            command_id: command,
            param_overrides: vec![
                ModuleCommandParamOverride {
                    param_id: address,
                    value: ParamValue::Str("/override".into()),
                },
                ModuleCommandParamOverride {
                    param_id: argument,
                    value: ParamValue::Int(42),
                },
            ],
            invocation_id: None,
            delivery_policy: ModuleCommandDeliveryPolicy::Standard,
        })
        .unwrap(),
    );
    let invocation = CapturedInvocation {
        snapshot: fixture.snapshot().try_lease().unwrap(),
        execution: Some(event.try_lease_payload(4096).unwrap()),
        command,
        module: fixture.module,
        override_weight: 4096,
    };
    let prepared = prepare(OutboundRequest::command(invocation.clone(), fixture.producer())).unwrap();
    let message = decode(&prepared.bytes);
    assert_eq!(message.addr, "/override");
    assert_eq!(message.args, [rosc::OscType::Int(42)]);
    // An override cannot turn a non-parameter argument into a valid source.
    fixture.nodes.get_mut(&argument).unwrap().param_value = None;
    let invalid = CapturedInvocation {
        snapshot: fixture.snapshot().try_lease().unwrap(),
        ..invocation
    };
    let error = prepare(OutboundRequest::command(invalid, fixture.producer()))
        .err()
        .unwrap();
    assert!(error.contains("not a parameter"), "{error}");
}

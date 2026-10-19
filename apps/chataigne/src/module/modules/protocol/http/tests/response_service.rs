use super::*;

#[test]
fn http_rejected_graph_edits_report_failure_without_replaying_the_response() {
    use golden_core::{edit::Edit, process_ctx::ProcessCtx};

    let (mut engine, module_id) = create_http_module();
    let snapshot = engine.process_tree_snapshot();
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    let crate::app::AppNode::HttpModule(module) = engine.nodes.get_mut(module_id).unwrap() else {
        panic!("HTTP module")
    };
    module.enqueue_response_for_test(test_response(
        "rejected",
        Some("application/json"),
        br#"{"value":42}"#.to_vec(),
    ));
    module.service_pending_events(&mut ctx, Some(&snapshot));
    assert!(!ctx.edits.pending.is_empty());

    // Scheduled callbacks accumulate in one queue before stabilization applies it.
    // Model a rejected request from an earlier callback followed by this HTTP turn.
    engine.edits.push(Edit::SetParam {
        node: NodeId(u64::MAX),
        value: ParamValue::Int(1),
        behaviour: golden_core::parameter::ParameterEventBehaviour::Append,
    });
    engine.absorb_edits(&mut ctx).unwrap();
    assert!(engine.apply_edits().is_err());
    assert!(find_path(&engine, module_id, "values/rejected/value").is_none());

    run_http_until_drained(&mut engine, module_id, 16);
    assert_eq!(callbacks(&engine), ["requestFailed"]);
    assert!(find_path(&engine, module_id, "values/rejected/value").is_none());
    run_http_runtime_ticks(&mut engine, 3);
    assert_eq!(
        callbacks(&engine),
        ["requestFailed"],
        "never retry endpoint effects or graph pulses"
    );
}

#[test]
fn http_partial_graph_failure_never_replays_an_applied_trigger_prefix() {
    use golden_core::{
        edit::{Edit, EditRequest},
        process_ctx::ProcessCtx,
    };

    let (mut engine, module_id) = create_http_module();
    let crate::app::AppNode::HttpModule(module) = engine.nodes.get_mut(module_id).unwrap() else {
        panic!("HTTP module")
    };
    module.enqueue_response_for_test(test_response(
        "partial",
        Some("application/json"),
        br#"{"pulse":[]}"#.to_vec(),
    ));
    run_http_until_drained(&mut engine, module_id, 16);
    let pulse = find_path(&engine, module_id, "values/partial/pulse").unwrap();
    let pulses_before = pulse_count(&engine, pulse);
    let snapshot = engine.process_tree_snapshot();
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    let crate::app::AppNode::HttpModule(module) = engine.nodes.get_mut(module_id).unwrap() else {
        panic!("HTTP module")
    };
    module.enqueue_response_for_test(test_response(
        "partial",
        Some("application/json"),
        br#"{"pulse":[]," pulse":[]}"#.to_vec(),
    ));
    module.service_pending_events(&mut ctx, Some(&snapshot));
    assert_eq!(ctx.edits.pending.len(), 2, "two ordered pulses");
    ctx.edits.pending.insert(
        1,
        EditRequest::new(Edit::SetParam {
            node: NodeId(u64::MAX),
            value: ParamValue::Int(1),
            behaviour: golden_core::parameter::ParameterEventBehaviour::Append,
        }),
    );
    engine.absorb_edits(&mut ctx).unwrap();
    assert!(engine.apply_edits().is_err());
    assert_eq!(pulse_count(&engine, pulse), pulses_before + 1);
    run_http_until_drained(&mut engine, module_id, 16);
    run_http_runtime_ticks(&mut engine, 3);
    assert_eq!(pulse_count(&engine, pulse), pulses_before + 1);
    assert_eq!(callbacks(&engine), ["responseReceived", "requestFailed"]);
}

fn pulse_count(engine: &crate::app::AppEngine, pulse: NodeId) -> usize {
    engine
        .ui_event_log()
        .iter()
        .filter(|event| matches!(event.kind, EventKind::ParamChanged { param, .. } if param == pulse))
        .count()
}

#[test]
fn http_retains_callback_ownership_when_a_shared_queue_discards_delivery() {
    use golden_core::{edit::Edit, process_ctx::ProcessCtx};

    for reject_before in [true, false] {
        let (mut engine, module_id) = create_http_module();
        let snapshot = engine.process_tree_snapshot();
        let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
        let crate::app::AppNode::HttpModule(module) = engine.nodes.get_mut(module_id).unwrap() else {
            panic!("HTTP module")
        };
        module.enqueue_response_for_test(test_response("delivery", None, Vec::new()));
        module.service_pending_events(&mut ctx, Some(&snapshot));
        assert!(ctx
            .edits
            .pending
            .iter()
            .any(|request| matches!(request.edit, Edit::EmitCustomEvent { .. })));
        let rejected = Edit::SetParam {
            node: NodeId(u64::MAX),
            value: ParamValue::Int(1),
            behaviour: golden_core::parameter::ParameterEventBehaviour::Append,
        };
        if reject_before {
            engine.edits.push(rejected);
        } else {
            ctx.edits.push(rejected);
        }
        engine.absorb_edits(&mut ctx).unwrap();
        assert!(engine.apply_edits().is_err());
        assert_eq!(callbacks(&engine).is_empty(), reject_before);
        run_http_until_drained(&mut engine, module_id, 16);
        assert_eq!(callbacks(&engine), ["responseReceived"]);
        run_http_runtime_ticks(&mut engine, 3);
        assert_eq!(callbacks(&engine), ["responseReceived"]);
    }
}

#[test]
fn http_waits_for_edit_receipts_and_publishes_callbacks_without_tree_snapshots() {
    use golden_core::process_ctx::ProcessCtx;

    let (mut engine, module_id) = create_http_module();
    let time = engine.time;
    let snapshot = engine.process_tree_snapshot();
    let mut values = ProcessCtx::new(ExecutionPhase::EngineTick, time);
    let mut waiting = ProcessCtx::new(ExecutionPhase::EngineTick, time);
    let crate::app::AppNode::HttpModule(module) = engine.nodes.get_mut(module_id).unwrap() else {
        panic!("HTTP module")
    };
    module.enqueue_response_for_test(test_response(
        "receipt",
        Some("application/json"),
        br#"{"value":7}"#.to_vec(),
    ));
    module.service_pending_events(&mut values, Some(&snapshot));
    assert!(
        !module.update_requires_tree_snapshot(),
        "a pending receipt cannot plan another turn"
    );
    module.update(&mut waiting);
    assert!(
        waiting.edits.pending.is_empty(),
        "never advance a cursor before its edits apply"
    );
    engine.absorb_edits(&mut values).unwrap();
    engine.apply_edits().unwrap();

    let mut delivery = ProcessCtx::new(ExecutionPhase::EngineTick, time);
    let crate::app::AppNode::HttpModule(module) = engine.nodes.get_mut(module_id).unwrap() else {
        panic!("HTTP module")
    };
    assert!(
        !module.update_requires_tree_snapshot(),
        "final application acknowledgement needs no graph copy"
    );
    module.update(&mut delivery);
    let queued = delivery.edits.pending.len();
    assert_eq!(queued, 1);
    module.update(&mut delivery);
    assert_eq!(
        delivery.edits.pending.len(),
        queued,
        "do not duplicate an unpublished callback"
    );
    engine.absorb_edits(&mut delivery).unwrap();
    engine.apply_edits().unwrap();

    let crate::app::AppNode::HttpModule(module) = engine.nodes.get_mut(module_id).unwrap() else {
        panic!("HTTP module")
    };
    assert!(!module.update_requires_tree_snapshot());
    module.update(&mut waiting);
    assert!(!module.has_pending_responses_for_test());
    assert_eq!(callbacks(&engine), ["responseReceived"]);
    let value = find_path(&engine, module_id, "values/receipt/value").unwrap();
    assert_eq!(param_value(&engine, value), ParamValue::Int(7));
}

#[test]
fn http_response_materialization_is_bounded_per_turn_and_preserves_large_lists() {
    let (mut engine, module_id) = create_http_module();
    let before = engine.nodes.len();
    let body = serde_json::to_vec(&json!({ "values": (0..1024).collect::<Vec<_>>() })).unwrap();
    let crate::app::AppNode::HttpModule(module) = engine.nodes.get_mut(module_id).unwrap() else {
        panic!("HTTP module")
    };
    module.enqueue_response_for_test(test_response("budget", Some("application/json"), body));
    run_http_runtime_ticks(&mut engine, 1);
    assert!(
        engine.nodes.len() - before <= 256,
        "one HTTP response must not monopolize graph materialization"
    );
    assert!(
        response_received_callback_payload(&engine).is_none(),
        "completion must wait for all values"
    );
    run_http_until_drained(&mut engine, module_id, 128);
    let last = find_path(&engine, module_id, "values/budget/values/value 1024").unwrap();
    assert_eq!(param_value(&engine, last), ParamValue::Int(1023));
    assert!(
        engine.nodes.get(last).unwrap().node_data().meta.description.is_none(),
        "indexed values must not duplicate response provenance"
    );
    assert!(response_received_callback_payload(&engine).is_some());
    assert_no_duplicate_child_keys(&engine, find_path(&engine, module_id, "values").unwrap());
}

#[test]
fn http_list_shrink_is_bounded_and_preserves_unindexed_children_and_survivor_identity() {
    let (mut engine, module_id) = create_http_module();
    enqueue_list(&mut engine, module_id, 1024);
    run_http_until_drained(&mut engine, module_id, 128);
    let list = find_path(&engine, module_id, "values/budget/values").unwrap();
    let first = find_path(&engine, list, "value 1").unwrap();
    engine.add_node(Folder::new("metadata").into(), Some(list));
    engine.apply_edits().unwrap();
    engine.resolve().unwrap();
    let metadata = find_path(&engine, list, "metadata").unwrap();
    enqueue_list(&mut engine, module_id, 10);
    let mut finished = false;
    for turn in 0..128 {
        let before = engine.nodes.len();
        run_http_runtime_ticks(&mut engine, 1);
        assert!(
            before.saturating_sub(engine.nodes.len()) <= 256,
            "tail removal must have a service budget"
        );
        if turn == 0 {
            // Delete the next remaining tail externally while the response is suspended.
            let mut child = engine.nodes.get(list).unwrap().node_data().first_child;
            let marker = loop {
                let id = child.expect("remaining tail");
                let data = engine.nodes.get(id).unwrap().node_data();
                if data
                    .meta
                    .label
                    .strip_prefix("value ")
                    .and_then(|text| text.parse::<usize>().ok())
                    .is_some_and(|index| index > 10)
                {
                    break id;
                }
                child = data.next_sibling;
            };
            engine.edits.push(golden_core::edit::Edit::RemoveNode { node: marker });
            engine.apply_edits().unwrap();
            engine.resolve().unwrap();
        }
        let crate::app::AppNode::HttpModule(module) = engine.nodes.get(module_id).unwrap() else {
            panic!("HTTP module")
        };
        if !module.has_pending_responses_for_test() {
            finished = true;
            break;
        }
    }
    assert!(finished);
    assert_eq!(find_path(&engine, list, "value 1"), Some(first));
    assert!(engine.nodes.get(metadata).is_some());
    assert_eq!(direct_child_count(&engine, list), 11);
    assert!(find_path(&engine, list, "value 11").is_none());
}

#[test]
fn http_failure_completion_cannot_overtake_a_materializing_response() {
    let (mut engine, module_id) = create_http_module();
    enqueue_list(&mut engine, module_id, 1024);
    let response = test_response("later failure", None, Vec::new());
    let mut failure = super::super::transport::HttpFailure {
        origin: response.origin,
        request: response.request,
        error: "injected failure".into(),
        attempts: 1,
        callback_args: Vec::new(),
    };
    failure.callback_args = super::super::callbacks::prepare_failure_callback(&failure);
    let crate::app::AppNode::HttpModule(module) = engine.nodes.get_mut(module_id).unwrap() else {
        panic!("HTTP module")
    };
    module
        .pending_events
        .push_back(super::super::PendingHttpEvent::Failure(failure));
    run_http_runtime_ticks(&mut engine, 1);
    assert!(callbacks(&engine).is_empty());
    run_http_until_drained(&mut engine, module_id, 128);
    assert_eq!(callbacks(&engine), ["responseReceived", "requestFailed"]);
}

#[test]
fn http_overlapping_paths_and_repeated_triggers_settle_in_input_order() {
    let (mut engine, module_id) = create_http_module();
    // Trimming JSON keys maps both entries to the same domain path. Preserve both pulses.
    let body = br#"{"pulse":[]," pulse":[]}"#.to_vec();
    let crate::app::AppNode::HttpModule(module) = engine.nodes.get_mut(module_id).unwrap() else {
        panic!("HTTP module")
    };
    module.enqueue_response_for_test(test_response("ordered", Some("application/json"), body));
    run_http_until_drained(&mut engine, module_id, 32);
    let pulse = find_path(&engine, module_id, "values/ordered/pulse").unwrap();
    let emitted = engine
        .ui_event_log()
        .iter()
        .filter(|event| matches!(event.kind, EventKind::ParamChanged { param, .. } if param == pulse))
        .count();
    assert_eq!(emitted, 2, "both pulses must survive creation of their target");

    let body = br#"{"a":4," a":{"child":8}}"#.to_vec();
    let crate::app::AppNode::HttpModule(module) = engine.nodes.get_mut(module_id).unwrap() else {
        panic!("HTTP module")
    };
    module.enqueue_response_for_test(test_response("ordered", Some("application/json"), body));
    run_http_until_drained(&mut engine, module_id, 32);
    let expected = find_path(&engine, module_id, "values/ordered/a").unwrap();
    // serde_json sorts object keys: the prefixed key's subtree precedes the scalar.
    assert_eq!(param_value(&engine, expected), ParamValue::Int(4));
}

#[test]
fn http_single_value_larger_than_the_turn_byte_budget_makes_progress() {
    let (mut engine, module_id) = create_http_module();
    let text = "x".repeat(100_000);
    let body = serde_json::to_vec(&json!({"text": text})).unwrap();
    let crate::app::AppNode::HttpModule(module) = engine.nodes.get_mut(module_id).unwrap() else {
        panic!("HTTP module")
    };
    module.enqueue_response_for_test(test_response("large", Some("application/json"), body));
    run_http_until_drained(&mut engine, module_id, 16);
    let value = find_path(&engine, module_id, "values/large/text").unwrap();
    assert_eq!(param_value(&engine, value), ParamValue::Str(text));
    assert_eq!(callbacks(&engine), ["responseReceived"]);
}

fn enqueue_list(engine: &mut crate::app::AppEngine, module_id: NodeId, count: usize) {
    let body = serde_json::to_vec(&json!({ "values": (0..count).collect::<Vec<_>>() })).unwrap();
    let crate::app::AppNode::HttpModule(module) = engine.nodes.get_mut(module_id).unwrap() else {
        panic!("HTTP module")
    };
    module.enqueue_response_for_test(test_response("budget", Some("application/json"), body));
}

fn callbacks(engine: &crate::app::AppEngine) -> Vec<&str> {
    engine
        .ui_event_log()
        .iter()
        .filter_map(|event| {
            let EventKind::Custom(custom) = &event.kind else {
                return None;
            };
            if custom.topic != crate::app::module::script_api::MODULE_SCRIPT_CALLBACK_TOPIC {
                return None;
            }
            custom.payload.get("callback").and_then(serde_json::Value::as_str)
        })
        .filter(|name| matches!(*name, "responseReceived" | "requestFailed"))
        .collect()
}

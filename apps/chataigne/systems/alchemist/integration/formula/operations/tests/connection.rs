use super::*;

fn endpoint(engine: &crate::app::AppEngine, node: NodeId, socket: &str) -> FormulaSocketDto {
    FormulaSocketDto {
        node,
        uuid: engine.nodes.get(node).unwrap().node_data().meta.uuid,
        socket: socket.into(),
    }
}

fn connect(
    engine: &mut crate::app::AppEngine,
    formula: &FormulaOwnerDto,
    from: NodeId,
    from_socket: &str,
    to: NodeId,
    to_socket: &str,
) -> FormulaConnectResult {
    let request = FormulaConnectRequest {
        formula: formula.clone(),
        from: endpoint(engine, from, from_socket),
        to: endpoint(engine, to, to_socket),
    };
    invoke(engine, FORMULA_CONNECT_OPERATION, request).unwrap()
}

#[test]
fn connection_replacement_is_idempotent_and_one_undo_step_with_exact_identity() {
    let (mut engine, formula) = fixture();
    let a = anode(&mut engine, &formula, "constant");
    let b = anode(&mut engine, &formula, "constant");
    let target = anode(&mut engine, &formula, "math");
    let first = connect(&mut engine, &formula, a, "value", target, "value1");
    assert!(first.replaced.is_empty());
    let history = engine.undo_len();
    let same = connect(&mut engine, &formula, a, "value", target, "value1");
    assert_eq!(same.connection.node, first.connection.node);
    assert_eq!(same.connection.uuid, first.connection.uuid);
    assert_eq!(engine.undo_len(), history);
    let second = connect(&mut engine, &formula, b, "value", target, "value1");
    assert_eq!(second.replaced.len(), 1);
    assert_eq!(second.replaced[0].uuid, first.connection.uuid);
    assert!(engine.nodes.get(first.connection.node).is_none());
    assert_eq!(
        engine.nodes.get(second.connection.node).unwrap().node_data().meta.uuid,
        second.connection.uuid
    );
    assert_eq!(engine.undo_len(), history + 1);
    assert!(apply_project_intent(&mut engine, UiEditIntent::Undo).success);
    assert_eq!(
        engine.nodes.get(first.connection.node).unwrap().node_data().meta.uuid,
        first.connection.uuid
    );
    assert!(engine.nodes.get(second.connection.node).is_none());
    assert!(apply_project_intent(&mut engine, UiEditIntent::Redo).success);
    assert_eq!(
        engine.nodes.get(second.connection.node).unwrap().node_data().meta.uuid,
        second.connection.uuid
    );
}

#[test]
fn invalid_connection_requests_preserve_the_existing_input_and_history() {
    let (mut engine, formula) = fixture();
    let source = anode(&mut engine, &formula, "constant");
    let target = anode(&mut engine, &formula, "gate");
    let other = anode(&mut engine, &formula, "math");
    let old = connect(&mut engine, &formula, source, "value", other, "value1");
    let count = engine.nodes.len();
    let history = engine.undo_len();
    let events = engine.ui_event_log().len();
    for mode in ["missing_port", "incompatible", "stale_endpoint", "wrong_owner"] {
        let mut request = FormulaConnectRequest {
            formula: formula.clone(),
            from: endpoint(&engine, source, "value"),
            to: endpoint(&engine, other, "value1"),
        };
        match mode {
            "missing_port" => request.from.socket = "missing".into(),
            "incompatible" => request.to = endpoint(&engine, target, "trigger"),
            "stale_endpoint" => request.from.uuid = NodeUuid(uuid::Uuid::new_v4()),
            "wrong_owner" => request.from = endpoint(&engine, formula.node, "value"),
            _ => unreachable!(),
        }
        let ack = invoke::<FormulaConnectResult>(&mut engine, FORMULA_CONNECT_OPERATION, request).unwrap_err();
        assert!(ack.result.is_none(), "{mode}");
        assert_eq!(engine.nodes.len(), count, "{mode}");
        assert_eq!(engine.undo_len(), history, "{mode}");
        assert_eq!(engine.ui_event_log().len(), events, "{mode}");
        assert!(engine.nodes.get(old.connection.node).is_some(), "{mode}");
    }
}

#[test]
fn vector_component_inputs_coexist_and_replace_whole_or_matching_components_only() {
    let (mut engine, formula) = fixture();
    let a = anode(&mut engine, &formula, "constant");
    let b = anode(&mut engine, &formula, "constant");
    let vector = anode(&mut engine, &formula, "pack_vec3");
    let target = anode(&mut engine, &formula, "extract_vec3");
    let x = connect(&mut engine, &formula, a, "value", target, "value.x");
    let y = connect(&mut engine, &formula, b, "value", target, "value.y");
    assert!(y.replaced.is_empty());
    let replacement_x = connect(&mut engine, &formula, b, "value", target, "value.x");
    assert_eq!(replacement_x.replaced[0].uuid, x.connection.uuid);
    assert!(engine.nodes.get(y.connection.node).is_some());
    let whole = connect(&mut engine, &formula, a, "value", target, "value");
    assert_eq!(whole.replaced.len(), 2);
    let component = connect(&mut engine, &formula, vector, "value.z", target, "value.z");
    assert_eq!(component.replaced.len(), 1);
    assert_eq!(component.replaced[0].uuid, whole.connection.uuid);
    let graph = formula_from_snapshot(&engine.process_tree_snapshot(), formula.node).unwrap();
    assert_eq!(graph.graph.edges().len(), 1);
}

#[test]
fn generic_input_constraints_are_solved_before_publishing_a_connection() {
    let (mut engine, formula) = fixture();
    let trigger = anode(&mut engine, &formula, "trigger_on_off");
    let scalar = anode(&mut engine, &formula, "constant");
    let target = anode(&mut engine, &formula, "compare");
    let existing = connect(&mut engine, &formula, trigger, "on", target, "left");
    let nodes = engine.nodes.len();
    let history = engine.undo_len();
    let events = engine.ui_event_log().len();
    let request = FormulaConnectRequest {
        formula,
        from: endpoint(&engine, scalar, "value"),
        to: endpoint(&engine, target, "right"),
    };
    let ack = invoke::<FormulaConnectResult>(&mut engine, FORMULA_CONNECT_OPERATION, request).unwrap_err();
    assert!(ack.result.is_none());
    assert_eq!(engine.nodes.len(), nodes);
    assert_eq!(engine.undo_len(), history);
    assert_eq!(engine.ui_event_log().len(), events);
    assert_eq!(
        engine
            .nodes
            .get(existing.connection.node)
            .unwrap()
            .node_data()
            .meta
            .uuid,
        existing.connection.uuid
    );
}

#[test]
fn inferred_vector_components_connect_and_grow_dynamic_inputs_without_ui_repair() {
    let (mut engine, formula) = fixture();
    let vector = anode(&mut engine, &formula, "pack_vec3");
    let scalar = anode(&mut engine, &formula, "constant");
    let math = anode(&mut engine, &formula, "math");
    let target = anode(&mut engine, &formula, "extract_vec3");
    connect(&mut engine, &formula, vector, "value", math, "value2");
    connect(&mut engine, &formula, scalar, "value", math, "value1.x");
    connect(&mut engine, &formula, math, "result.z", target, "value.x");
    let snapshot = engine.process_tree_snapshot();
    let config = snapshot.find_child_by_decl_id(math, "config").unwrap();
    assert_eq!(
        child_param(&snapshot, config, "config/num_inputs"),
        Some(&ParamValue::Int(3))
    );
    let materialized = formula_from_snapshot(&snapshot, formula.node).unwrap();
    assert_eq!(materialized.graph.edges().len(), 3);
    let solved = solve_document_types(
        &materialized.graph,
        &TypeSolveCtx {
            nodes: registry(),
            value_types: value_types(),
            properties: Some(&materialized.properties),
        },
    );
    assert!(!solved.has_errors(), "{:?}", solved.diagnostics);
}

#[test]
fn a_generic_default_scalar_does_not_admit_a_component_without_a_vector_binding() {
    let (mut engine, formula) = fixture();
    let scalar = anode(&mut engine, &formula, "constant");
    let math = anode(&mut engine, &formula, "math");
    let nodes = engine.nodes.len();
    let history = engine.undo_len();
    let request = FormulaConnectRequest {
        formula,
        from: endpoint(&engine, scalar, "value"),
        to: endpoint(&engine, math, "value1.x"),
    };
    let ack = invoke::<FormulaConnectResult>(&mut engine, FORMULA_CONNECT_OPERATION, request).unwrap_err();
    assert!(ack.result.is_none());
    assert_eq!(engine.nodes.len(), nodes);
    assert_eq!(engine.undo_len(), history);
}

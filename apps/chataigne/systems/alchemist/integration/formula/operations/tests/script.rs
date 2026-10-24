use super::*;
use golden_core::{
    application::service_script_app_operations,
    script::{ScriptNode, ScriptNodeConfig, ScriptSource},
};

pub(super) fn attach_script(engine: &mut crate::app::AppEngine, parent: NodeId, source: String) -> NodeId {
    let mut script = ScriptNode::new(
        "Formula authoring script",
        ScriptNodeConfig {
            source: ScriptSource::Inline { text: source },
        },
    );
    script.budgets.max_wall_time_us_per_load = 1_000_000;
    script.budgets.max_wall_time_us_per_callback = 1_000_000;
    let uuid = script.node_data().meta.uuid;
    engine.add_node(script.into(), Some(parent));
    engine.apply_edits().unwrap();
    engine.node_id_by_uuid(uuid).unwrap()
}

fn replies(engine: &crate::app::AppEngine) -> Vec<serde_json::Value> {
    engine
        .ui_event_log()
        .iter()
        .filter_map(|event| match &event.kind {
            EventKind::Custom(event) if event.topic == "formula-script-reply" => Some((*event.payload).clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn embedded_script_copies_and_pastes_through_the_same_complete_fragment_operation() {
    let (mut engine, formula, source, target) = connected_fixture();
    let owner = serde_json::to_string(&formula).unwrap();
    let nodes = serde_json::to_string(&vec![source, target]).unwrap();
    attach_script(
        &mut engine,
        formula.node,
        format!(
            r#"
        function init() {{
            const owner = {{node: local.id(), uuid: local.uuid}};
            gc.invokeAppOperation('copy', '{FORMULA_OPERATION_NAMESPACE}', '{FORMULA_COPY_OPERATION}', {{formula: owner, nodes: {nodes}}});
        }}
        function onAppOperationResult(reply) {{
            if (reply.request_id === 'copy' && reply.status === 'applied') {{
                gc.invokeAppOperation('paste', '{FORMULA_OPERATION_NAMESPACE}', '{FORMULA_PASTE_OPERATION}', {{
                    formula: {owner}, fragment: reply.value, offset: [5,7]
                }});
            }} else {{ gc.emit('formula-script-reply', reply); }}
        }}
    "#
        ),
    );
    let history = engine.undo_len();
    service_script_app_operations(&mut engine).unwrap();
    assert!(
        replies(&engine).is_empty(),
        "callback-created paste must wait for the next service turn"
    );
    service_script_app_operations(&mut engine).unwrap();
    let reply = replies(&engine).pop().unwrap();
    assert_eq!(reply["status"], "applied");
    let pasted: FormulaPasteResult = serde_json::from_value(reply["value"].clone()).unwrap();
    assert_eq!(pasted.selected_nodes.len(), 2);
    for identity in &pasted.identities {
        assert_eq!(
            engine.nodes.get(identity.node).unwrap().node_data().meta.uuid,
            identity.uuid
        );
    }
    assert_eq!(
        engine.undo_len(),
        history + 1,
        "copy and callbacks must not introduce authored history steps"
    );
    assert!(apply_project_intent(&mut engine, UiEditIntent::Undo).success);
    assert!(
        pasted
            .identities
            .iter()
            .all(|identity| !engine.nodes.contains(identity.node))
    );
    assert!(apply_project_intent(&mut engine, UiEditIntent::Redo).success);
    assert!(
        pasted
            .identities
            .iter()
            .all(|identity| engine.nodes.get(identity.node).unwrap().node_data().meta.uuid == identity.uuid)
    );
}

#[test]
fn embedded_script_connection_returns_exact_identity_and_an_invalid_followup_is_atomic() {
    let (mut engine, formula) = fixture();
    let source = anode(&mut engine, &formula, "constant");
    let target = anode(&mut engine, &formula, "math");
    let request = FormulaConnectRequest {
        formula: formula.clone(),
        from: FormulaSocketDto {
            node: source,
            uuid: engine.nodes.get(source).unwrap().node_data().meta.uuid,
            socket: "value".into(),
        },
        to: FormulaSocketDto {
            node: target,
            uuid: engine.nodes.get(target).unwrap().node_data().meta.uuid,
            socket: "value1".into(),
        },
    };
    let request = serde_json::to_string(&request).unwrap();
    let root = engine.root;
    attach_script(
        &mut engine,
        root,
        format!(
            r#"
        function init() {{ gc.invokeAppOperation('connect', '{FORMULA_OPERATION_NAMESPACE}', '{FORMULA_CONNECT_OPERATION}', {request}); }}
        function onAppOperationResult(reply) {{
            gc.emit('formula-script-reply', reply);
            if (reply.request_id === 'connect' && reply.status === 'applied') {{
                const invalid = {request}; invalid.to.socket = 'missing-socket';
                gc.invokeAppOperation('invalid', '{FORMULA_OPERATION_NAMESPACE}', '{FORMULA_CONNECT_OPERATION}', invalid);
            }}
        }}
    "#
        ),
    );
    let history = engine.undo_len();
    service_script_app_operations(&mut engine).unwrap();
    let first: FormulaConnectResult = serde_json::from_value(replies(&engine)[0]["value"].clone()).unwrap();
    let nodes = engine.nodes.len();
    service_script_app_operations(&mut engine).unwrap();
    let replies = replies(&engine);
    assert_eq!(replies[1]["status"], "rejected");
    assert!(replies[1].get("value").is_none());
    assert_eq!(engine.nodes.len(), nodes);
    assert_eq!(engine.undo_len(), history + 1);
    assert_eq!(
        engine.nodes.get(first.connection.node).unwrap().node_data().meta.uuid,
        first.connection.uuid
    );
}

#[test]
fn fragment_paste_preserves_a_complete_authored_script_child() {
    let (mut engine, formula) = fixture();
    let node = anode(&mut engine, &formula, "constant");
    let script = attach_script(&mut engine, node, "function init() {}\nfunction update() {}".into());
    let script_uuid = engine.nodes.get(script).unwrap().node_data().meta.uuid;
    let source_data = engine.nodes.get(script).unwrap().project_encode_data().unwrap();
    let fragment = copy(&mut engine, &formula, vec![node]);
    let pasted: FormulaPasteResult = invoke(
        &mut engine,
        FORMULA_PASTE_OPERATION,
        FormulaPasteRequest {
            formula,
            fragment,
            offset: None,
        },
    )
    .unwrap();
    let imported = pasted
        .identities
        .iter()
        .find(|identity| identity.source_uuid == script_uuid)
        .unwrap();
    assert_eq!(
        engine.nodes.get(imported.node).unwrap().project_encode_data().unwrap(),
        source_data
    );
}

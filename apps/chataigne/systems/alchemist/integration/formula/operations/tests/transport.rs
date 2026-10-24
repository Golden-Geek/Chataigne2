use super::*;
use golden_core::ui_sync::{UiNodeDataDto, UiSnapshot};
use std::time::{Duration, Instant};

mod history;
mod host;
use host::Host;

fn operation(name: &str, payload: impl Serialize) -> UiEditIntent {
    UiEditIntent::InvokeAppOperation {
        namespace: FORMULA_OPERATION_NAMESPACE.into(),
        operation: name.into(),
        payload: serde_json::to_value(payload).unwrap(),
    }
}
fn result<R: DeserializeOwned>(ack: UiAck) -> R {
    assert!(ack.success, "{ack:?}");
    serde_json::from_value(ack.result.expect("authoritative result").value).unwrap()
}
fn owner(snapshot: &UiSnapshot, uuid: NodeUuid) -> FormulaOwnerDto {
    FormulaOwnerDto {
        node: snapshot.nodes.iter().find(|node| node.uuid == uuid).unwrap().node_id,
        uuid,
    }
}
fn assert_imported(snapshot: &UiSnapshot, pasted: &FormulaPasteResult) {
    for identity in &pasted.identities {
        assert_eq!(
            snapshot
                .nodes
                .iter()
                .find(|node| node.node_id == identity.node)
                .unwrap()
                .uuid,
            identity.uuid
        );
    }
}

#[test]
#[ignore = "requires cargo build --locked -p Chataigne2 first; starts only an isolated loopback headless child"]
fn real_http_websocket_and_embedded_script_share_formula_operations() {
    let (engine, formula, source, target) = connected_fixture();
    let source_uuid = engine.nodes.get(source).unwrap().node_data().meta.uuid;
    let target_uuid = engine.nodes.get(target).unwrap().node_data().meta.uuid;
    let contents = engine.to_project_json_with(Node::project_encode_data).unwrap();
    let host = Host::start();
    host.upload(contents);
    let snapshot = host.snapshot();
    let formula = owner(&snapshot, formula.uuid);
    let source = owner(&snapshot, source_uuid);
    let target = owner(&snapshot, target_uuid);
    let fragment: FormulaFragment = result(host.intent(&operation(
        FORMULA_COPY_OPERATION,
        FormulaCopyRequest {
            formula: formula.clone(),
            nodes: vec![source.node, target.node],
        },
    )));
    let pasted: FormulaPasteResult = result(host.intent(&operation(
        FORMULA_PASTE_OPERATION,
        FormulaPasteRequest {
            formula: formula.clone(),
            fragment: fragment.clone(),
            offset: Some([5.0, 7.0]),
        },
    )));
    assert_imported(&host.snapshot(), &pasted);
    assert!(host.intent(&UiEditIntent::Undo).success);
    let undone = host.snapshot();
    assert!(
        pasted
            .identities
            .iter()
            .all(|identity| !undone.nodes.iter().any(|node| node.node_id == identity.node))
    );
    assert!(host.intent(&UiEditIntent::Redo).success);
    assert_imported(&host.snapshot(), &pasted);

    let mut socket = host.websocket();
    let from = pasted
        .identities
        .iter()
        .find(|identity| identity.source_uuid == source_uuid)
        .unwrap();
    let request = FormulaConnectRequest {
        formula: formula.clone(),
        from: FormulaSocketDto {
            node: from.node,
            uuid: from.uuid,
            socket: "value".into(),
        },
        to: FormulaSocketDto {
            node: target.node,
            uuid: target.uuid,
            socket: "value1".into(),
        },
    };
    let connected: FormulaConnectResult = result(Host::ws_intent(
        &mut socket,
        "connect",
        operation(FORMULA_CONNECT_OPERATION, request.clone()),
    ));
    assert_eq!(connected.replaced.len(), 1);
    let before = host.snapshot();
    assert_eq!(
        before
            .nodes
            .iter()
            .find(|node| node.node_id == connected.connection.node)
            .unwrap()
            .uuid,
        connected.connection.uuid
    );
    let mut invalid = request;
    invalid.to.socket = "missing-socket".into();
    let rejected = Host::ws_intent(
        &mut socket,
        "invalid-connect",
        operation(FORMULA_CONNECT_OPERATION, invalid),
    );
    assert!(!rejected.success && rejected.result.is_none());
    let after = host.snapshot();
    assert_eq!(before.nodes.len(), after.nodes.len());
    assert_eq!(before.history, after.history);
    socket.close(None).unwrap();

    let mut jobs = Vec::new();
    for index in 0..8 {
        let base = host.base.clone();
        let client = host.client.clone();
        let intent = operation(
            FORMULA_PASTE_OPERATION,
            FormulaPasteRequest {
                formula: formula.clone(),
                fragment: fragment.clone(),
                offset: Some([index as f64, 20.0]),
            },
        );
        jobs.push(std::thread::spawn(move || {
            let ack: UiAck = client
                .post(format!("{base}/api/ui/intent"))
                .json(&intent)
                .send()
                .unwrap()
                .error_for_status()
                .unwrap()
                .json()
                .unwrap();
            result::<FormulaPasteResult>(ack)
        }));
    }
    let mut ids = HashSet::new();
    let mut uuids = HashSet::new();
    let mut results = Vec::new();
    for job in jobs {
        let pasted = job.join().unwrap();
        for identity in &pasted.identities {
            assert!(ids.insert(identity.node));
            assert!(uuids.insert(identity.uuid));
        }
        results.push(pasted);
    }
    let snapshot = host.snapshot();
    for pasted in results {
        assert_imported(&snapshot, &pasted);
    }

    let (mut fresh, fresh_formula, _, _) = connected_fixture();
    let reply = parameter("script_reply", "Script reply", ParamValue::Str(String::new()), false);
    let reply_uuid = reply.node_data().meta.uuid;
    fresh.add_node(reply.into(), Some(fresh_formula.node));
    fresh.apply_edits().unwrap();
    super::script::attach_script(
        &mut fresh,
        fresh_formula.node,
        format!(
            r#"
        function init() {{
            const nodes = local.getChildren().filter(node => node.type === 'alchemist_anode').map(node => node.id());
            gc.invokeAppOperation('native-copy','{FORMULA_OPERATION_NAMESPACE}','{FORMULA_COPY_OPERATION}', {{
                formula: {{node:local.id(),uuid:local.uuid}}, nodes
            }});
        }}
        function onAppOperationResult(reply) {{
            if (reply.request_id === 'native-copy' && reply.status === 'applied') {{
                gc.invokeAppOperation('native-paste','{FORMULA_OPERATION_NAMESPACE}','{FORMULA_PASTE_OPERATION}', {{
                    formula: {{node:local.id(),uuid:local.uuid}}, fragment:reply.value, offset:[5,7]
                }});
            }} else {{ local.getChild('script_reply').value = JSON.stringify(reply); }}
        }}
    "#
        ),
    );
    host.upload(fresh.to_project_json_with(Node::project_encode_data).unwrap());
    let deadline = Instant::now() + Duration::from_secs(20);
    let final_snapshot = loop {
        let snapshot = host.snapshot();
        let node = snapshot.nodes.iter().find(|node| node.uuid == reply_uuid).unwrap();
        if let UiNodeDataDto::Parameter { param } = &node.data {
            if let ParamValue::Str(text) = &param.value {
                if !text.is_empty() {
                    let reply: serde_json::Value = serde_json::from_str(text).unwrap();
                    assert_eq!(reply["status"], "applied", "{reply}");
                    assert_eq!(reply["request_id"], "native-paste");
                    let pasted: FormulaPasteResult = serde_json::from_value(reply["value"].clone()).unwrap();
                    assert_imported(&snapshot, &pasted);
                    break snapshot;
                }
            }
        }
        assert!(
            Instant::now() < deadline,
            "explicit script reply deadline; evidence {}",
            host.evidence.display()
        );
        std::thread::sleep(Duration::from_millis(20));
    };
    let stale = host.intent(&operation(
        FORMULA_PASTE_OPERATION,
        FormulaPasteRequest {
            formula,
            fragment,
            offset: None,
        },
    ));
    assert!(!stale.success && stale.result.is_none());
    let unchanged = host.snapshot();
    assert_eq!(unchanged.nodes.len(), final_snapshot.nodes.len());
    assert_eq!(unchanged.history, final_snapshot.history);
}

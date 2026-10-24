use super::*;
use golden_core::application::ProductionRuntime;
use std::{sync::Arc, thread};

#[test]
fn simultaneous_pastes_and_independent_creates_return_disjoint_exact_records() {
    let (mut engine, formula) = fixture();
    let source = anode(&mut engine, &formula, "constant");
    let fragment = copy(&mut engine, &formula, vec![source]);
    let runtime = Arc::new(ProductionRuntime::new(engine, golden_core::ui_sync::UiProjectFileSpec::default()).unwrap());
    let mut jobs = Vec::new();
    for index in 0..8 {
        let paste_runtime = runtime.clone();
        let request = FormulaPasteRequest {
            formula: formula.clone(),
            fragment: fragment.clone(),
            offset: Some([index as f64, 10.0]),
        };
        jobs.push(thread::spawn(move || {
            let ack = paste_runtime
                .apply_ui_transaction(
                    UiEditIntent::InvokeAppOperation {
                        namespace: FORMULA_OPERATION_NAMESPACE.into(),
                        operation: FORMULA_PASTE_OPERATION.into(),
                        payload: serde_json::to_value(request).unwrap(),
                    },
                    Some("paste-client"),
                )
                .unwrap()
                .acknowledgement;
            assert!(ack.success, "{ack:?}");
            Some(serde_json::from_value::<FormulaPasteResult>(ack.result.unwrap().value).unwrap())
        }));
        let create_runtime = runtime.clone();
        let parent = formula.node;
        jobs.push(thread::spawn(move || {
            let ack = create_runtime
                .apply_ui_transaction(
                    UiEditIntent::CreateUserItem {
                        parent,
                        node_type: format!("{ANODE_CREATE_PREFIX}constant"),
                        label: Some("Independent create".into()),
                        initial_params: vec![],
                    },
                    Some("create-client"),
                )
                .unwrap()
                .acknowledgement;
            assert!(ack.success, "{ack:?}");
            None
        }));
    }
    let results = jobs
        .into_iter()
        .filter_map(|job| job.join().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(results.len(), 8);
    let mut uuids = HashSet::new();
    let mut nodes = HashSet::new();
    for result in &results {
        for identity in &result.identities {
            assert!(uuids.insert(identity.uuid));
            assert!(nodes.insert(identity.node));
        }
    }
    let snapshot = runtime.read_model().current_snapshot();
    let published = snapshot
        .nodes
        .iter()
        .map(|node| (node.node_id, node))
        .collect::<HashMap<_, _>>();
    for result in results {
        for identity in result.identities {
            assert_eq!(published[&identity.node].uuid, identity.uuid);
        }
    }
    assert_eq!(
        published[&formula.node]
            .children
            .iter()
            .filter(|id| published[id].node_type == ANODE_NODE_TYPE)
            .count(),
        17
    );
}

#[test]
fn replacement_project_reusing_live_ids_rejects_the_old_owner_and_accepts_a_fresh_intent() {
    let (mut source, owner) = fixture();
    let node = anode(&mut source, &owner, "constant");
    let fragment = copy(&mut source, &owner, vec![node]);
    let (mut replacement, fresh_owner) = fixture();
    assert_eq!(owner.node, fresh_owner.node);
    assert_ne!(owner.uuid, fresh_owner.uuid);
    let count = replacement.nodes.len();
    let ack = invoke::<FormulaPasteResult>(
        &mut replacement,
        FORMULA_PASTE_OPERATION,
        FormulaPasteRequest {
            formula: owner,
            fragment: fragment.clone(),
            offset: None,
        },
    )
    .unwrap_err();
    assert!(ack.result.is_none());
    assert_eq!(replacement.nodes.len(), count);
    let pasted: FormulaPasteResult = invoke(
        &mut replacement,
        FORMULA_PASTE_OPERATION,
        FormulaPasteRequest {
            formula: fresh_owner,
            fragment,
            offset: None,
        },
    )
    .unwrap();
    assert_eq!(pasted.selected_nodes.len(), 1);
    for identity in pasted.identities {
        assert_eq!(
            replacement.nodes.get(identity.node).unwrap().node_data().meta.uuid,
            identity.uuid
        );
    }
}

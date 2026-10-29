use super::*;
use crate::app::AppNode;
use crate::media::{asset, capture_channels, channel, validate_project};
use golden_core::app::{ProjectNode, from_sparse_project_json, to_sparse_project_json_pretty};
use golden_core::node::Node;

fn relative(path: &str) -> AssetLocator {
    AssetLocator::ProjectRelative {
        path: path.to_owned().try_into().unwrap(),
    }
}

fn import(engine: &mut AppEngine, path: &str) -> AssetId {
    let MediaEditResult::AssetCreated { asset } = apply(
        engine,
        MediaEdit::ImportAsset {
            locator: relative(path),
            label: None,
        },
    ) else {
        panic!("asset import result")
    };
    asset
}

#[test]
fn asset_relink_removal_history_and_sparse_full_codecs_keep_one_persistent_identity() {
    let mut engine = fixture();
    let before = engine.undo_len();
    let id = import(&mut engine, "media/original.png");
    assert_eq!(engine.undo_len(), before + 1);
    let original = asset(&engine, id).unwrap();
    assert_eq!(original.id, id);
    let node = engine.node_id_by_uuid(golden_core::model::NodeUuid(id.get())).unwrap();
    assert_eq!(engine.nodes.get(node).unwrap().node_data().meta.uuid.0, id.get());
    apply(
        &mut engine,
        MediaEdit::RelinkAsset {
            asset: id,
            locator: relative("moved/relinked.png"),
        },
    );
    let relinked = asset(&engine, id).unwrap();
    assert_eq!(relinked.id, id);
    assert_ne!(relinked.locator, original.locator);
    assert!(engine.undo().unwrap());
    assert_eq!(asset(&engine, id).unwrap(), original);
    assert!(engine.redo().unwrap());
    assert_eq!(asset(&engine, id).unwrap(), relinked);
    apply(&mut engine, MediaEdit::RemoveAsset { asset: id });
    assert!(asset(&engine, id).is_err());
    assert!(engine.undo().unwrap());
    assert_eq!(asset(&engine, id).unwrap(), relinked);

    let sparse = to_sparse_project_json_pretty(&engine).unwrap();
    for forbidden in ["sha256", "fingerprint", "pixels", "shared_handle", "retained_bytes"] {
        assert!(!sparse.contains(forbidden), "runtime field persisted: {forbidden}");
    }
    let mut sparse_loaded = from_sparse_project_json::<AppNode>(&sparse).unwrap();
    sparse_loaded.apply_edits().unwrap();
    validate_project(&sparse_loaded).unwrap();
    assert_eq!(asset(&sparse_loaded, id).unwrap(), relinked);

    let full = engine.to_project_file_with(Node::project_encode_data).unwrap();
    let mut full_loaded = AppEngine::from_project_file_with(full, AppNode::project_decode_node).unwrap();
    full_loaded.apply_edits().unwrap();
    validate_project(&full_loaded).unwrap();
    assert_eq!(asset(&full_loaded, id).unwrap(), relinked);
}

#[test]
fn asset_intents_validate_locator_and_exact_catalog_owner_atomically() {
    let mut engine = fixture();
    let reference = create(&mut engine, MediaKind::Picture);
    let invalid_id = AssetId::from_uuid(reference.channel.0.0);
    let history = engine.undo_len();
    let original = to_sparse_project_json_pretty(&engine).unwrap();
    assert!(
        !invoke(
            &mut engine,
            MediaEdit::RelinkAsset {
                asset: invalid_id,
                locator: relative("asset.png")
            }
        )
        .success
    );
    let invalid_locator = apply_project_intent(
        &mut engine,
        UiEditIntent::InvokeAppOperation {
            namespace: MEDIA_NAMESPACE.into(),
            operation: MEDIA_OPERATION.into(),
            payload: serde_json::json!({ "kind": "ImportAsset", "locator": { "kind": "project_relative", "path": "../escape.png" }, "label": null }),
        },
    );
    assert!(!invalid_locator.success);
    assert_eq!(engine.undo_len(), history);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), original);
}

#[test]
fn asset_removal_relink_and_undo_fence_captures_even_when_history_retains_the_node() {
    let mut engine = fixture();
    let id = import(&mut engine, "source.png");
    let reference = create(&mut engine, MediaKind::Picture);
    let mut definition = channel(&engine, reference).unwrap().definition;
    definition.source = ChannelSource::Picture { asset: Some(id) };
    apply(
        &mut engine,
        MediaEdit::ConfigureChannel {
            reference,
            definition: definition.clone(),
        },
    );
    let captured = capture_channels(&engine, &[reference], 16).unwrap();
    apply(
        &mut engine,
        MediaEdit::RelinkAsset {
            asset: id,
            locator: relative("replacement.png"),
        },
    );
    assert!(!captured.is_current());
    let captured = capture_channels(&engine, &[reference], 16).unwrap();
    apply(&mut engine, MediaEdit::RemoveAsset { asset: id });
    assert!(
        !captured.is_current(),
        "history-retained asset must stop authorizing old preparation"
    );
    assert_eq!(channel(&engine, reference).unwrap().definition, definition);
    let missing = capture_channels(&engine, &[reference], 16).unwrap();
    assert!(engine.undo().unwrap());
    assert!(
        !missing.is_current(),
        "undo-restored missing asset must require a new capture"
    );
    assert_eq!(asset(&engine, id).unwrap().locator, relative("replacement.png"));
    let captured = capture_channels(&engine, &[reference], 16).unwrap();
    let intent = UiEditIntent::MoveNode {
        node: engine.node_id_by_uuid(golden_core::model::NodeUuid(id.get())).unwrap(),
        new_parent: engine.node_id_by_uuid(reference.media.0).unwrap(),
        new_prev_sibling: None,
    };
    let ack = apply_project_intent(&mut engine, intent);
    assert!(ack.success, "{ack:?}");
    assert!(
        !captured.is_current(),
        "generic catalog move must fence captured source ownership"
    );
    assert!(asset(&engine, id).is_err());
}

#[test]
fn quickjs_asset_import_uses_the_committed_backend_identity_and_history_route() {
    use golden_core::application::service_script_app_operations;
    use golden_core::events::EventKind;
    use golden_core::script::{ScriptNode, ScriptNodeConfig, ScriptSource};
    let mut engine = fixture();
    let request = serde_json::to_string(&MediaEdit::ImportAsset {
        locator: relative("picture.png"),
        label: None,
    })
    .unwrap();
    let source = format!(
        "function init() {{ gc.invokeAppOperation('import', {MEDIA_NAMESPACE:?}, {MEDIA_OPERATION:?}, {request}); }}\nfunction onAppOperationResult(reply) {{ gc.emit('asset-reply', reply); }}"
    );
    let mut script = ScriptNode::new(
        "Asset Caller",
        ScriptNodeConfig {
            source: ScriptSource::Inline { text: source },
        },
    );
    script.budgets.max_wall_time_us_per_load = 1_000_000;
    script.budgets.max_wall_time_us_per_callback = 1_000_000;
    engine.add_node(script.into(), None);
    engine.apply_edits().unwrap();
    let before = engine.undo_len();
    service_script_app_operations(&mut engine).unwrap();
    let reply = engine
        .ui_event_log()
        .iter()
        .find_map(|event| match &event.kind {
            EventKind::Custom(event) if event.topic == "asset-reply" => Some((*event.payload).clone()),
            _ => None,
        })
        .expect("committed asset callback");
    assert_eq!(reply["status"], "applied");
    let MediaEditResult::AssetCreated { asset: id } = serde_json::from_value(reply["value"].clone()).unwrap() else {
        panic!("asset import result")
    };
    assert_eq!(asset(&engine, id).unwrap().locator, relative("picture.png"));
    assert_eq!(engine.undo_len(), before + 1);
    assert!(engine.undo().unwrap());
    assert!(asset(&engine, id).is_err());
    assert!(engine.redo().unwrap());
    assert_eq!(asset(&engine, id).unwrap().id, id);
}

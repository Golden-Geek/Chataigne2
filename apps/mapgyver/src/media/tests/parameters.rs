use super::*;
use crate::{app::AppNode, media::channel};
use golden_core::{
    app::{from_sparse_project_json, to_sparse_project_json_pretty},
    node::{Node, NodeId},
    parameter::{ParamValue, ParameterEventBehaviour},
};
use std::collections::HashMap;

fn parameters(engine: &AppEngine, root: NodeId) -> HashMap<String, NodeId> {
    let mut child = engine.nodes.get(root).unwrap().node_data().first_child;
    while let Some(id) = child {
        let node = engine.nodes.get(id).unwrap();
        if crate::parameters::is_parameter_folder(node) {
            let mut values = HashMap::new();
            let mut parameter = node.node_data().first_child;
            while let Some(id) = parameter {
                let node = engine.nodes.get(id).unwrap();
                assert!(node.engine_param_snapshot().is_some());
                values.insert(node.node_data().meta.decl_id.0.clone(), id);
                parameter = node.node_data().next_sibling;
            }
            return values;
        }
        child = node.node_data().next_sibling;
    }
    panic!("missing declared Parameters folder")
}

fn set(engine: &mut AppEngine, node: NodeId, value: ParamValue) -> UiAck {
    apply_project_intent(
        engine,
        UiEditIntent::SetParam {
            node,
            value,
            behaviour: ParameterEventBehaviour::Append,
        },
    )
}

#[test]
fn media_channel_parameters_are_the_document_and_hidden_settings_keep_stable_identity() {
    let mut engine = fixture();
    let mut references = Vec::new();
    for kind in [
        MediaKind::SolidColor,
        MediaKind::Picture,
        MediaKind::SharedTexture,
        MediaKind::Composition,
        MediaKind::Matrix,
    ] {
        let reference = create(&mut engine, kind);
        let media = engine.node_id_by_uuid(reference.media.0).unwrap();
        let root = engine.node_id_by_uuid(reference.channel.0).unwrap();
        let media_fields = parameters(&engine, media);
        assert_eq!(media_fields.len(), 2);
        let resolution = media_fields["parameters/resolution"];
        let fields = parameters(&engine, root);
        assert_eq!(fields.len(), 17, "{fields:?}");
        assert!(engine.nodes.get(root).unwrap().project_encode_data().unwrap().is_null());
        assert!(
            engine
                .nodes
                .get(media)
                .unwrap()
                .project_encode_data()
                .unwrap()
                .is_null()
        );
        let history = engine.undo_len();
        // Disabled Resolution retains its authored pixels independently of source-native sizing.
        assert!(set(&mut engine, resolution, ParamValue::Vec2(640.0, 1080.0)).success);
        assert_eq!(
            channel(&engine, reference).unwrap().definition.size,
            SizePolicy::Automatic
        );
        assert!(
            invoke(
                &mut engine,
                MediaEdit::ConfigureMediaResolution {
                    media: reference.media,
                    size: SizePolicy::Fixed {
                        extent: golden_video::Extent2D::new(640, 1080).unwrap()
                    }
                }
            )
            .success
        );
        assert_eq!(
            channel(&engine, reference).unwrap().definition.size,
            SizePolicy::Fixed {
                extent: golden_video::Extent2D::new(640, 1080).unwrap()
            }
        );
        assert_eq!(engine.undo_len(), history + 2);
        assert!(engine.undo().unwrap());
        assert_eq!(
            channel(&engine, reference).unwrap().definition.size,
            SizePolicy::Automatic
        );
        assert!(engine.redo().unwrap());
        assert_eq!(parameters(&engine, root), fields);
        let saved = to_sparse_project_json_pretty(&engine).unwrap();
        let undo = engine.undo_len();
        for (node, value) in [
            (resolution, ParamValue::Vec2(0.0, 1080.0)),
            (fields["parameters/source_kind"], ParamValue::Enum("Picture".into())),
        ] {
            if kind == MediaKind::Picture && node == fields["parameters/source_kind"] {
                continue;
            }
            assert!(!set(&mut engine, node, value).success);
            assert_eq!(engine.undo_len(), undo);
            assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), saved);
        }
        references.push(reference);
    }
    let snapshot = crate::media::catalog::inspect(&engine).unwrap();
    let json = to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded = from_sparse_project_json::<AppNode>(&json).unwrap();
    crate::media::validate_project(&loaded).unwrap();
    assert_eq!(crate::media::catalog::inspect(&loaded).unwrap(), snapshot);
    for reference in references {
        assert!(channel(&loaded, reference).is_ok());
    }
    // Actual generic writes remain viable after load, with identical identities.
    let first = snapshot.media[0].channels[0].reference;
    let root = loaded.node_id_by_uuid(first.channel.0).unwrap();
    let field = parameters(&loaded, root)["parameters/filter"];
    assert!(set(&mut loaded, field, ParamValue::Enum("Nearest".into())).success);
    assert_eq!(
        channel(&loaded, first).unwrap().definition.filter,
        golden_video::plan::SampleFilter::Nearest
    );
}

#[test]
fn asset_locator_parameters_relink_and_reject_traversal_with_history_and_roundtrip() {
    let mut engine = fixture();
    let MediaEditResult::AssetCreated { asset } = apply(
        &mut engine,
        MediaEdit::ImportAsset {
            locator: AssetLocator::ProjectRelative {
                path: "images/original.png".to_owned().try_into().unwrap(),
            },
            label: None,
        },
    ) else {
        panic!()
    };
    let root = engine
        .node_id_by_uuid(golden_core::node::NodeUuid(asset.get()))
        .unwrap();
    let fields = parameters(&engine, root);
    assert_eq!(fields.len(), 3);
    let path = fields["parameters/project_path"];
    assert!(set(&mut engine, path, ParamValue::File("images/replacement.png".into())).success);
    let record = crate::media::asset(&engine, asset).unwrap();
    assert_eq!(
        record.locator,
        AssetLocator::ProjectRelative {
            path: "images/replacement.png".to_owned().try_into().unwrap()
        }
    );
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    assert!(!set(&mut engine, path, ParamValue::File("../escape.png".into())).success);
    assert_eq!(engine.undo_len(), history);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), saved);
    assert!(engine.undo().unwrap());
    assert_ne!(crate::media::asset(&engine, asset).unwrap(), record);
    assert!(engine.redo().unwrap());
    let loaded = from_sparse_project_json::<AppNode>(&saved).unwrap();
    assert_eq!(crate::media::asset(&loaded, asset).unwrap(), record);
}

#[test]
fn resolution_generic_edits_toggle_all_variants_and_reject_invalid_pixels_atomically() {
    let mut engine = fixture();
    let first = create(&mut engine, MediaKind::Picture);
    let MediaEditResult::Created { reference: second } = apply(
        &mut engine,
        MediaEdit::AddChannel {
            media: first.media,
            label: None,
        },
    ) else {
        panic!()
    };
    let root = engine.node_id_by_uuid(first.media.0).unwrap();
    let resolution = parameters(&engine, root)["parameters/resolution"];
    let parameter_uuid = engine.nodes.get(resolution).unwrap().node_data().meta.uuid;
    let node = engine.nodes.get(resolution).unwrap();
    assert!(!node.node_data().meta.enabled);
    assert!(node.node_data().meta.can_be_disabled);
    assert_eq!(node.engine_param_snapshot().unwrap().constraints.step, Some(1.0));
    let captured = crate::media::capture_channels(&engine, &[first, second], 4).unwrap();
    assert!(set(&mut engine, resolution, ParamValue::Vec2(640.0, 360.0)).success);
    assert!(!captured.is_current());
    assert_eq!(channel(&engine, first).unwrap().definition.size, SizePolicy::Automatic);
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    for value in [
        ParamValue::Vec2(0.0, 1.0),
        ParamValue::Vec2(1.0, 16385.0),
        ParamValue::Vec2(1.5, 2.0),
        ParamValue::Vec2(f64::NAN, 2.0),
        ParamValue::Vec2(2.0, f64::INFINITY),
        ParamValue::Str("640x360".into()),
    ] {
        assert!(!set(&mut engine, resolution, value).success);
        assert_eq!(engine.undo_len(), history);
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
    }
    let captured = crate::media::capture_channels(&engine, &[first, second], 4).unwrap();
    let ack = apply_project_intent(
        &mut engine,
        UiEditIntent::PatchMeta {
            node: resolution,
            patch: golden_core::ui_sync::NodeMetaPatch {
                enabled: Some(true),
                ..Default::default()
            },
        },
    );
    assert!(ack.success, "{ack:?}");
    assert!(!captured.is_current(), "An enable-only edit must wake rendering");
    let fixed = SizePolicy::Fixed {
        extent: golden_video::Extent2D::new(640, 360).unwrap(),
    };
    for reference in [first, second] {
        assert_eq!(channel(&engine, reference).unwrap().definition.size, fixed);
    }
    assert_eq!(engine.undo_len(), history + 1);
    assert!(engine.undo().unwrap());
    assert_eq!(channel(&engine, first).unwrap().definition.size, SizePolicy::Automatic);
    assert!(engine.redo().unwrap());
    let encoded = to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded = from_sparse_project_json::<AppNode>(&encoded).unwrap();
    let loaded_resolution = loaded.node_id_by_uuid(parameter_uuid).unwrap();
    for reference in [first, second] {
        assert_eq!(
            channel(&loaded, reference).unwrap(),
            channel(&engine, reference).unwrap()
        );
    }
    apply(
        &mut loaded,
        MediaEdit::ConfigureMediaResolution {
            media: first.media,
            size: SizePolicy::Automatic,
        },
    );
    assert_eq!(
        loaded
            .nodes
            .get(loaded_resolution)
            .unwrap()
            .engine_param_snapshot()
            .unwrap()
            .value,
        ParamValue::Vec2(640.0, 360.0)
    );
    assert_eq!(channel(&loaded, second).unwrap().definition.size, SizePolicy::Automatic);
    let mut definition = channel(&loaded, second).unwrap().definition;
    definition.size = fixed;
    let before = to_sparse_project_json_pretty(&loaded).unwrap();
    assert!(
        !invoke(
            &mut loaded,
            MediaEdit::ConfigureChannel {
                reference: second,
                definition
            }
        )
        .success
    );
    assert_eq!(to_sparse_project_json_pretty(&loaded).unwrap(), before);
}

#[test]
fn malformed_parameter_records_are_rejected_before_a_project_can_run() {
    let mut resolution_engine = fixture();
    let reference = create(&mut resolution_engine, MediaKind::SolidColor);
    let media = resolution_engine.node_id_by_uuid(reference.media.0).unwrap();
    let resolution = parameters(&resolution_engine, media)["parameters/resolution"];
    for value in [
        serde_json::json!([0, 1080]),
        serde_json::json!([16385, 1080]),
        serde_json::json!([1920.5, 1080]),
    ] {
        assert_invalid_record(&resolution_engine, resolution, value);
    }
    for (name, value) in [
        ("parameters/canvas_width", serde_json::json!(-1)),
        ("parameters/source_kind", serde_json::json!("unknown")),
    ] {
        let mut engine = fixture();
        let reference = create(&mut engine, MediaKind::Composition);
        let root = engine.node_id_by_uuid(reference.channel.0).unwrap();
        let field = parameters(&engine, root)[name];
        assert_invalid_record(&engine, field, value);
    }
    let mut engine = fixture();
    let MediaEditResult::AssetCreated { asset } = apply(
        &mut engine,
        MediaEdit::ImportAsset {
            locator: AssetLocator::ProjectRelative {
                path: "images/source.png".to_owned().try_into().unwrap(),
            },
            label: None,
        },
    ) else {
        panic!()
    };
    let root = engine
        .node_id_by_uuid(golden_core::node::NodeUuid(asset.get()))
        .unwrap();
    let field = parameters(&engine, root)["parameters/project_path"];
    assert_invalid_record(&engine, field, serde_json::json!("../invalid.png"));
}

fn assert_invalid_record(engine: &AppEngine, parameter: NodeId, value: serde_json::Value) {
    use golden_core::{app::ProjectNode, engine::ProjectNodeRecord};
    let uuid = engine.nodes.get(parameter).unwrap().node_data().meta.uuid;
    let mut project = engine.to_project_file_with(Node::project_encode_data).unwrap();
    fn corrupt(record: &mut ProjectNodeRecord, uuid: golden_core::node::NodeUuid, value: &serde_json::Value) -> bool {
        if record.uuid == uuid {
            record.data = Some(serde_json::json!({ "value": value }));
            return true;
        }
        record.children.iter_mut().any(|child| corrupt(child, uuid, value))
    }
    assert!(corrupt(&mut project.root, uuid, &value));
    let result = AppEngine::from_project_file_with(project, AppNode::project_decode_node)
        .map_err(|error| error.to_string())
        .and_then(|loaded| golden_core::app::validate_engine_project_candidate(&loaded));
    assert!(
        result.is_err(),
        "malformed persisted Parameter value {value} must be rejected"
    );
}

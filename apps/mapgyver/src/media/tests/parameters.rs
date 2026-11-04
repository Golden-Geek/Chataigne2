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
        assert_eq!(
            media_fields.len(),
            match kind {
                MediaKind::SharedTexture => 5,
                MediaKind::Picture => 4,
                _ => 3,
            }
        );
        let resolution = media_fields["parameters/resolution"];
        let fields = parameters(&engine, root);
        assert_eq!(
            fields.len(),
            if kind == MediaKind::SharedTexture { 7 } else { 6 },
            "{fields:?}"
        );
        for removed in [
            "maximum_width",
            "maximum_height",
            "pixel_format",
            "color_encoding",
            "alpha_mode",
            "canvas_width",
            "canvas_height",
            "empty_cell",
        ] {
            assert!(!fields.contains_key(&format!("parameters/{removed}")));
        }
        assert!(!fields.contains_key("parameters/filter"));
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
    let root = loaded.node_id_by_uuid(first.media.0).unwrap();
    let field = parameters(&loaded, root)["parameters/resolution"];
    assert!(set(&mut loaded, field, ParamValue::Vec2(800.0, 600.0)).success);
    assert_eq!(
        channel(&loaded, first).unwrap().definition.size,
        SizePolicy::Fixed {
            extent: golden_video::Extent2D::new(800, 600).unwrap()
        }
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
    assert_eq!(fields.len(), 1);
    let path = fields["parameters/file"];
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
fn one_asset_file_switches_location_and_clears_all_shared_consumers_without_changing_identity() {
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
    let references = [
        create(&mut engine, MediaKind::Picture),
        create(&mut engine, MediaKind::Picture),
    ];
    for reference in references {
        apply(
            &mut engine,
            MediaEdit::ConfigureChannel {
                reference,
                source: ChannelSource::Picture { asset: Some(asset) },
            },
        );
    }
    let root = engine
        .node_id_by_uuid(golden_core::node::NodeUuid(asset.get()))
        .unwrap();
    let fields = parameters(&engine, root);
    assert_eq!(fields.len(), 1);
    let file = fields["parameters/file"];
    let uuid = engine.nodes.get(file).unwrap().node_data().meta.uuid;
    let original = crate::media::asset(&engine, asset).unwrap();
    let captured = crate::media::capture_channels(&engine, &references, 16).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let absolute = directory.path().join("replacement.png").to_str().unwrap().to_owned();
    let history = engine.undo_len();
    assert!(set(&mut engine, file, ParamValue::File(absolute.clone())).success);
    assert_eq!(engine.undo_len(), history + 1);
    assert!(!captured.is_current());
    let replacement = crate::media::asset(&engine, asset).unwrap();
    assert_eq!(
        replacement.locator,
        AssetLocator::Absolute {
            path: absolute.try_into().unwrap()
        }
    );
    assert!(engine.undo().unwrap());
    assert_eq!(crate::media::asset(&engine, asset).unwrap(), original);
    assert!(engine.redo().unwrap());
    assert_eq!(crate::media::asset(&engine, asset).unwrap(), replacement);
    let active = crate::media::capture_channels(&engine, &references, 16).unwrap();
    assert_eq!(
        active.assets.len(),
        1,
        "shared channels retain one authoritative source"
    );
    assert!(set(&mut engine, file, ParamValue::File(String::new())).success);
    assert!(!active.is_current());
    assert!(crate::media::asset(&engine, asset).is_err());
    let cleared = crate::media::capture_channels(&engine, &references, 16).unwrap();
    assert!(cleared.assets.is_empty());
    let cleared_json = to_sparse_project_json_pretty(&engine).unwrap();
    let cleared_loaded = from_sparse_project_json::<AppNode>(&cleared_json).unwrap();
    assert!(crate::media::asset(&cleared_loaded, asset).is_err());
    assert!(
        cleared_loaded
            .node_id_by_uuid(golden_core::node::NodeUuid(asset.get()))
            .is_some()
    );
    for reference in references {
        assert_eq!(
            channel(&engine, reference).unwrap().definition.source,
            ChannelSource::Picture { asset: Some(asset) }
        );
    }
    assert!(engine.undo().unwrap());
    assert!(!cleared.is_current());
    apply(
        &mut engine,
        MediaEdit::RelinkAsset {
            asset,
            locator: original.locator.clone(),
        },
    );
    assert_eq!(parameters(&engine, root), fields);
    assert_eq!(engine.nodes.get(file).unwrap().node_data().meta.uuid, uuid);
    assert_eq!(crate::media::asset(&engine, asset).unwrap(), original);
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let loaded = from_sparse_project_json::<AppNode>(&saved).unwrap();
    assert_eq!(crate::media::asset(&loaded, asset).unwrap(), original);
    assert_eq!(
        parameters(
            &loaded,
            loaded
                .node_id_by_uuid(golden_core::node::NodeUuid(asset.get()))
                .unwrap()
        )
        .len(),
        1
    );
}

#[test]
fn retired_asset_schema_and_split_path_records_reject_before_recoverable_decode() {
    use golden_core::engine::ProjectNodeRecord;
    let mut engine = fixture();
    let MediaEditResult::AssetCreated { asset } = apply(
        &mut engine,
        MediaEdit::ImportAsset {
            locator: AssetLocator::ProjectRelative {
                path: "source.png".to_owned().try_into().unwrap(),
            },
            label: None,
        },
    ) else {
        panic!()
    };
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let old = saved.replace(ASSET_NODE_TYPE, "mapgyver.media.asset");
    assert_ne!(old, saved);
    assert!(from_sparse_project_json::<AppNode>(&old).is_err());
    let root = engine
        .node_id_by_uuid(golden_core::node::NodeUuid(asset.get()))
        .unwrap();
    let file = parameters(&engine, root)["parameters/file"];
    let uuid = engine.nodes.get(file).unwrap().node_data().meta.uuid;
    fn rename(record: &mut ProjectNodeRecord, uuid: golden_core::node::NodeUuid, decl: &str) -> bool {
        if record.uuid == uuid {
            record.meta.decl_id.as_mut().unwrap().0 = decl.to_owned();
            return true;
        }
        record.children.iter_mut().any(|child| rename(child, uuid, decl))
    }
    for retired in [
        "parameters/locator_kind",
        "parameters/project_path",
        "parameters/absolute_path",
    ] {
        let mut project = engine.to_project_file_with(Node::project_encode_data).unwrap();
        assert!(rename(&mut project.root, uuid, retired));
        assert!(
            super::super::persistence::validate_project_file(&project)
                .unwrap_err()
                .contains("one File")
        );
    }
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
    let before = to_sparse_project_json_pretty(&loaded).unwrap();
    assert!(
        serde_json::from_value::<MediaEdit>(serde_json::json!({
        "kind": "ConfigureChannel", "reference": second,
            "definition": {"source": {"kind": "Picture", "asset": null}, "size": fixed, "filter": "Linear"}
        }))
        .is_err()
    );
    assert_eq!(to_sparse_project_json_pretty(&loaded).unwrap(), before);
}

#[test]
fn sampling_is_not_authored_or_exposed_through_any_media_contract() {
    let mut engine = fixture();
    for kind in [
        MediaKind::SolidColor,
        MediaKind::Picture,
        MediaKind::SharedTexture,
        MediaKind::Composition,
        MediaKind::Matrix,
    ] {
        let reference = create(&mut engine, kind);
        let owner = engine.node_id_by_uuid(reference.media.0).unwrap();
        assert!(!parameters(&engine, owner).contains_key("parameters/filter"));
        let definition = serde_json::to_value(channel(&engine, reference).unwrap().definition).unwrap();
        assert!(definition.get("filter").is_none());
        assert!(
            serde_json::from_value::<MediaEdit>(serde_json::json!({
                "kind": "ConfigureMediaSampling", "media": reference.media, "filter": "Nearest"
            }))
            .is_err()
        );
    }
    let MediaEditResult::ReferenceCreated { media } = apply(
        &mut engine,
        MediaEdit::CreateReference {
            target: None,
            label: None,
        },
    ) else {
        panic!()
    };
    assert!(!parameters(&engine, engine.node_id_by_uuid(media.0).unwrap()).contains_key("parameters/filter"));
    assert!(!super::super::source_api::METHODS.contains(&"configureMediaSampling"));
    assert!(serde_json::from_value::<MediaCommandKind>(serde_json::json!("ConfigureSampling")).is_err());
}

#[test]
fn retired_sampling_records_reject_before_recoverable_project_decode() {
    use golden_core::engine::ProjectNodeRecord;
    let mut engine = fixture();
    let reference = create(&mut engine, MediaKind::SolidColor);
    let owner = engine.node_id_by_uuid(reference.media.0).unwrap();
    let resolution = parameters(&engine, owner)["parameters/resolution"];
    let uuid = engine.nodes.get(resolution).unwrap().node_data().meta.uuid;
    let mut project = engine.to_project_file_with(Node::project_encode_data).unwrap();
    fn rename(record: &mut ProjectNodeRecord, uuid: golden_core::node::NodeUuid) -> bool {
        if record.uuid == uuid {
            record.meta.decl_id.as_mut().unwrap().0 = "parameters/filter".into();
            return true;
        }
        record.children.iter_mut().any(|child| rename(child, uuid))
    }
    assert!(rename(&mut project.root, uuid));
    assert!(
        super::super::persistence::validate_project_file(&project)
            .unwrap_err()
            .contains("fixed to Linear")
    );
}

#[test]
fn procedural_resolution_and_matrix_settings_have_one_media_owner_through_history_and_reload() {
    use crate::matrix::protocol::*;
    for kind in [MediaKind::Composition, MediaKind::Matrix] {
        let mut engine = fixture();
        let reference = create(&mut engine, kind);
        let media = engine.node_id_by_uuid(reference.media.0).unwrap();
        let fields = parameters(&engine, media);
        let resolution = fields["parameters/resolution"];
        let mut references = vec![reference];
        if kind == MediaKind::Matrix {
            apply(
                &mut engine,
                MediaEdit::Matrix {
                    media: reference.media,
                    edit: MatrixEdit::AddLayers { parent: None, count: 1 },
                },
            );
            let MediaEditResult::Matrix { snapshot } = apply(
                &mut engine,
                MediaEdit::Matrix {
                    media: reference.media,
                    edit: MatrixEdit::Inspect,
                },
            ) else {
                panic!()
            };
            references.extend(snapshot.items.iter().map(|item| item.channel));
        }
        let canvas = |engine: &AppEngine| match channel(engine, reference).unwrap().definition.source {
            ChannelSource::Composition { canvas } => canvas,
            ChannelSource::Matrix {
                output: MatrixOutput::Final { definition },
            } => definition.canvas,
            _ => panic!(),
        };
        assert_eq!(canvas(&engine), crate::media::resolution::default_canvas());
        assert!(set(&mut engine, resolution, ParamValue::Vec2(800.0, 600.0)).success);
        assert_eq!(canvas(&engine), crate::media::resolution::default_canvas());
        let captured = crate::media::capture_channels(&engine, &references, 16).unwrap();
        let history = engine.undo_len();
        let extent = golden_video::Extent2D::new(640, 360).unwrap();
        apply(
            &mut engine,
            MediaEdit::ConfigureMediaResolution {
                media: reference.media,
                size: SizePolicy::Fixed { extent },
            },
        );
        assert!(!captured.is_current());
        assert_eq!(engine.undo_len(), history + 1);
        assert_eq!(canvas(&engine), extent);
        for reference in &references {
            assert_eq!(
                crate::media::authoring::channel_internal(&engine, *reference)
                    .unwrap()
                    .definition
                    .size,
                SizePolicy::Fixed { extent }
            );
            let root = engine.node_id_by_uuid(reference.channel.0).unwrap();
            let channel_fields = parameters(&engine, root);
            for name in ["canvas_width", "canvas_height", "empty_cell"] {
                assert!(!channel_fields.contains_key(&format!("parameters/{name}")));
            }
        }
        if kind == MediaKind::Matrix {
            let MediaEditResult::Matrix { snapshot } = apply(
                &mut engine,
                MediaEdit::Matrix {
                    media: reference.media,
                    edit: MatrixEdit::Inspect,
                },
            ) else {
                panic!()
            };
            assert_eq!(snapshot.definition.canvas, extent);
        }
        assert!(engine.undo().unwrap());
        assert_eq!(canvas(&engine), crate::media::resolution::default_canvas());
        assert!(engine.redo().unwrap());
        assert_eq!(canvas(&engine), extent);
        if kind == MediaKind::Matrix {
            assert!(!fields.contains_key("parameters/transition_ms"));
            assert!(!fields.contains_key("parameters/transition_curve"));
            for obsolete in [
                serde_json::json!({ "transition": { "duration_ms": 750, "curve": "SmoothStep" } }),
                serde_json::json!({ "canvas": { "width": 1, "height": 1 } }),
            ] {
                assert!(serde_json::from_value::<MatrixSettings>(obsolete).is_err());
            }
        }
        let saved = to_sparse_project_json_pretty(&engine).unwrap();
        let mut loaded = from_sparse_project_json::<AppNode>(&saved).unwrap();
        for reference in &references {
            assert_eq!(
                crate::media::authoring::channel_internal(&loaded, *reference).unwrap(),
                crate::media::authoring::channel_internal(&engine, *reference).unwrap()
            );
        }
        apply(
            &mut loaded,
            MediaEdit::ConfigureMediaResolution {
                media: reference.media,
                size: SizePolicy::Automatic,
            },
        );
        assert_eq!(canvas(&loaded), crate::media::resolution::default_canvas());
        let before = to_sparse_project_json_pretty(&loaded).unwrap();
        let source = channel(&loaded, reference).unwrap().definition.source;
        assert!(!invoke(&mut loaded, MediaEdit::ConfigureChannel { reference, source }).success);
        assert_eq!(to_sparse_project_json_pretty(&loaded).unwrap(), before);
    }
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
    let picture = create(&mut resolution_engine, MediaKind::Picture);
    let owner = resolution_engine.node_id_by_uuid(picture.media.0).unwrap();
    let fit = parameters(&resolution_engine, owner)["parameters/fit_mode"];
    assert_invalid_record(&resolution_engine, fit, serde_json::json!("Unknown"));
    {
        let (name, value) = ("parameters/source_kind", serde_json::json!("unknown"));
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
    let field = parameters(&engine, root)["parameters/file"];
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

use super::*;
use crate::app::{AppEngine, DmxSendFrameCommand};
use golden_core::{
    app::{ProjectLifecycle, ProjectNode},
    engine::{ProjectNodeMeta, ProjectNodeRecord},
    persistence::{decode_application_project_document, encode_application_project_document},
};

fn saved_legacy_project(sparse: bool) -> (String, Vec<golden_core::node::NodeUuid>) {
    let mut engine = AppEngine::new(Folder::new("root").into());
    for mut module in [
        AppNode::from(ArtNetModule::create()),
        AppNode::from(SacnModule::create()),
    ] {
        module.node_data_mut().meta.enabled = false;
        engine.add_node(module, None);
    }
    for _ in 0..8 {
        engine.apply_edits().unwrap();
    }
    let modules = engine
        .nodes
        .iter()
        .filter_map(|(id, node)| matches!(node.get_type(), "artnet_module" | "sacn_module").then_some(id))
        .collect::<Vec<_>>();
    for module in modules {
        let snapshot = engine.process_tree_snapshot();
        let tester = snapshot.find_child_by_decl_id(module, "command_tester").unwrap();
        engine.add_user_item(DmxSendFrameCommand::create().into(), Some(tester));
    }
    for _ in 0..8 {
        engine.apply_edits().unwrap();
    }
    let frames = engine
        .nodes
        .iter()
        .filter_map(|(id, node)| (node.get_type() == "bytes").then_some(id))
        .collect::<Vec<_>>();
    for frame in frames {
        engine.edits.push(golden_core::edit::Edit::SetParam {
            node: frame,
            value: ParamValue::Bytes(vec![1]),
            behaviour: golden_core::parameter::ParameterEventBehaviour::Coalesce,
        });
    }
    for _ in 0..8 {
        engine.apply_edits().unwrap();
    }
    let mut identities = Vec::new();
    let json = if sparse {
        let source = golden_core::app::to_sparse_project_json_pretty(&engine).unwrap();
        let mut document =
            decode_application_project_document::<ProjectNodeMeta>(&source, AppNode::application_id()).unwrap();
        legacy_frames(&mut document.project.root, &mut identities);
        encode_application_project_document(&document, AppNode::application_id()).unwrap()
    } else {
        let mut project = engine.to_project_file_with(|node| node.project_encode_data()).unwrap();
        legacy_frames(&mut project.root, &mut identities);
        serde_json::to_string(&project).unwrap()
    };
    (json, identities)
}

fn legacy_frames(record: &mut ProjectNodeRecord, identities: &mut Vec<golden_core::node::NodeUuid>) {
    // This fixture replaces only the historical DMX-owned parameter records.
    for child in &mut record.children {
        let decl = child
            .meta
            .decl_id
            .as_ref()
            .and_then(|id| id.0.rsplit('/').next())
            .unwrap_or("");
        if child.node_type == "bytes" && matches!(decl, "output_frame" | "slots") {
            child.node_type = "str".into();
            child.data = Some(serde_json::json!({
                "value": { "Str": "[0,127,255]" },
                "default_value": { "Str": "[]" }
            }));
            identities.push(child.uuid);
        }
        legacy_frames(child, identities);
    }
}

#[test]
fn legacy_dmx_frames_keep_values_and_identity_in_sparse_and_dense_projects() {
    for sparse in [false, true] {
        let (json, identities) = saved_legacy_project(sparse);
        assert_eq!(identities.len(), 4, "output and command frames in both protocols");
        let loaded = if sparse {
            golden_core::app::from_sparse_project_json::<AppNode>(&json).unwrap()
        } else {
            AppEngine::from_project_json_with(&json, AppNode::project_decode_node).unwrap()
        };
        for uuid in &identities {
            let node = loaded
                .nodes
                .iter()
                .find_map(|(_, node)| (node.node_data().meta.uuid == *uuid).then_some(node))
                .expect("original UUID survives");
            let parameter = node.engine_param_snapshot().unwrap();
            assert_eq!(node.get_type(), "bytes");
            assert_eq!(parameter.value, ParamValue::Bytes(vec![0, 127, 255]));
            assert_eq!(parameter.default_value, ParamValue::Bytes(vec![]));
        }
        let saved = golden_core::app::to_sparse_project_json_pretty(&loaded).unwrap();
        let reopened = golden_core::app::from_sparse_project_json::<AppNode>(&saved).unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&saved).unwrap(),
            serde_json::from_str::<serde_json::Value>(
                &golden_core::app::to_sparse_project_json_pretty(&reopened).unwrap()
            )
            .unwrap()
        );
    }
}

#[test]
fn frame_migration_is_narrow_bounded_and_idempotent() {
    use std::borrow::Cow;
    let mut legacy: ProjectNodeRecord = serde_json::from_value(serde_json::json!({
        "uuid": "00000000-0000-0000-0000-000000000007", "type": "str",
        "meta": { "decl_id": "slots" }, "data": { "value": { "Str": "[0,255]" } }
    }))
    .unwrap();
    let children = vec![legacy.clone()];
    let migrated = super::super::migrate_command_children(&children).unwrap().into_owned();
    assert_eq!(migrated[0].uuid, legacy.uuid);
    assert!(matches!(
        super::super::migrate_command_children(&migrated).unwrap(),
        Cow::Borrowed(_)
    ));
    legacy.data = Some(serde_json::json!({"value": {"Str": "[256]"}}));
    assert!(super::super::migrate_command_children(&[legacy.clone()]).is_err());
    legacy.meta.decl_id = Some(golden_core::node::DeclId("unrelated".into()));
    let unrelated = [legacy];
    assert!(matches!(
        super::super::migrate_command_children(&unrelated).unwrap(),
        Cow::Borrowed(_)
    ));
}

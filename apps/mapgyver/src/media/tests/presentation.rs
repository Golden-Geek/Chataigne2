use super::*;
use crate::app::AppNode;
use golden_core::{
    app::{ProjectNode, from_sparse_project_json, to_sparse_project_json_pretty},
    edit::Edit,
    node::{Node, NodeId, NodeMetaPatch, NodeUuid, PresentationHint},
    parameter::ParamValue,
    script::ScriptMethodCall,
};

fn patch(engine: &mut AppEngine, node: NodeId, presentation: PresentationHint) {
    let ack = apply_project_intent(
        engine,
        UiEditIntent::PatchMeta {
            node,
            patch: NodeMetaPatch {
                presentation: Some(presentation),
                ..Default::default()
            }
            .into(),
        },
    );
    assert!(ack.success, "{ack:?}");
}

#[test]
fn hosted_script_uses_public_color_methods_and_rejects_malformed_writes() {
    use golden_core::script::{ScriptNode, ScriptNodeConfig, ScriptSource};
    let mut engine = fixture();
    let MediaEditResult::Created { reference } = apply(
        &mut engine,
        MediaEdit::CreateMedia {
            media_kind: MediaKind::SolidColor,
            label: Some("Color Script Target".into()),
        },
    ) else {
        panic!()
    };
    let id = engine.node_id_by_uuid(reference.media.0).unwrap();
    let parent = super::super::authoring::manager(&engine).unwrap();
    let mut caller = ScriptNode::new(
        "Color caller",
        ScriptNodeConfig {
            source: ScriptSource::Inline {
                text: "function init() { local.getChild('Color Script Target').setColor(0.1, 0.2, 0.3, 1); }".into(),
            },
        },
    );
    caller.budgets.max_wall_time_us_per_load = 1_000_000;
    engine.add_node(caller.into(), Some(parent));
    engine.apply_edits().unwrap();
    engine.apply_edits().unwrap();
    let before = engine.nodes.get(id).unwrap().node_data().meta.presentation.clone();
    assert_eq!(
        serde_json::to_value(before.color).unwrap(),
        serde_json::json!({"r":0.1,"g":0.2,"b":0.3,"a":1.0})
    );
    for (method, args) in [
        ("setColor", vec![ParamValue::Float(0.5)]),
        (
            "setColor",
            vec![
                ParamValue::Bool(true),
                ParamValue::Int(0),
                ParamValue::Int(0),
                ParamValue::Int(1),
            ],
        ),
        (
            "setColor",
            vec![
                ParamValue::Float(f64::NAN),
                ParamValue::Int(0),
                ParamValue::Int(0),
                ParamValue::Int(1),
            ],
        ),
        ("resetColor", vec![ParamValue::Int(1)]),
    ] {
        engine.edits.push(Edit::CallNodeScriptMethod {
            node: id,
            call: ScriptMethodCall::prepare(method, args).unwrap(),
        });
        assert!(engine.apply_edits().is_err());
        assert_eq!(engine.nodes.get(id).unwrap().node_data().meta.presentation, before);
    }
}

fn script(engine: &mut AppEngine, node: NodeId, method: &str, args: Vec<ParamValue>) {
    engine.edits.push(Edit::CallNodeScriptMethod {
        node,
        call: ScriptMethodCall::prepare(method, args).unwrap(),
    });
    engine.apply_edits().unwrap();
    engine.apply_edits().unwrap();
}

#[test]
fn object_colors_share_typed_metadata_history_copy_reload_and_script_reset() {
    let mut engine = fixture();
    let media = create(&mut engine, MediaKind::SolidColor);
    let MediaEditResult::ReferenceCreated { media: reference } = apply(
        &mut engine,
        MediaEdit::CreateReference {
            target: Some(media.media),
            label: None,
        },
    ) else {
        panic!()
    };
    let MediaEditResult::FolderCreated { folder } = apply(
        &mut engine,
        MediaEdit::CreateFolder {
            parent: None,
            label: None,
        },
    ) else {
        panic!()
    };
    let MediaEditResult::FolderCreated { folder: nested } = apply(
        &mut engine,
        MediaEdit::CreateFolder {
            parent: Some(folder),
            label: None,
        },
    ) else {
        panic!()
    };
    let screen = crate::screens::inspect(&engine).unwrap().screens[0].clone();
    let original_media = super::super::channel(&engine, media).unwrap();
    let original_screen = screen.definition.clone();
    let color = serde_json::from_value(serde_json::json!({"r":0.8,"g":0.2,"b":0.45,"a":1.0})).unwrap();
    let uuids: [NodeUuid; 6] = [
        media.media.0,
        reference.0,
        folder,
        nested,
        screen.id.0,
        screen.surfaces[0].reference.surface.0,
    ];
    for uuid in uuids {
        let id = engine.node_id_by_uuid(uuid).unwrap();
        assert!(
            engine
                .nodes
                .get(id)
                .unwrap()
                .node_data()
                .meta
                .user_permissions
                .can_edit_color
        );
        let before = engine.nodes.get(id).unwrap().node_data().meta.presentation.clone();
        let history = engine.undo_len();
        let custom = PresentationHint {
            color: Some(color),
            ..before.clone()
        };
        patch(&mut engine, id, custom.clone());
        assert_eq!(engine.undo_len(), history + 1);
        assert!(engine.undo().unwrap());
        assert_eq!(engine.nodes.get(id).unwrap().node_data().meta.presentation, before);
        assert!(engine.redo().unwrap());
        assert_eq!(engine.nodes.get(id).unwrap().node_data().meta.presentation, custom);
        script(&mut engine, id, "resetColor", vec![]);
        assert_eq!(engine.nodes.get(id).unwrap().node_data().meta.presentation, before);
        script(
            &mut engine,
            id,
            "setColor",
            vec![
                ParamValue::Float(0.8),
                ParamValue::Float(0.2),
                ParamValue::Float(0.45),
                ParamValue::Int(1),
            ],
        );
        assert_eq!(engine.nodes.get(id).unwrap().node_data().meta.presentation, custom);
        let parent = engine.nodes.get(id).unwrap().node_data().parent.unwrap();
        let copied = engine
            .duplicate_subtree_with(
                id,
                parent,
                Some(id),
                None,
                Node::project_encode_data,
                AppNode::project_decode_node,
            )
            .unwrap();
        assert_eq!(
            engine.nodes.get(copied).unwrap().node_data().meta.presentation.color,
            Some(color)
        );
        script(&mut engine, copied, "resetColor", vec![]);
        assert_eq!(
            engine.nodes.get(id).unwrap().node_data().meta.presentation.color,
            Some(color)
        );
    }
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let loaded = from_sparse_project_json::<AppNode>(&saved).unwrap();
    for uuid in uuids {
        let node = loaded.nodes.get(loaded.node_id_by_uuid(uuid).unwrap()).unwrap();
        assert_eq!(node.node_data().meta.presentation.color, Some(color));
        assert!(node.node_data().meta.user_permissions.can_edit_color);
    }
    assert_eq!(super::super::channel(&engine, media).unwrap(), original_media);
    assert_eq!(
        crate::screens::inspect(&engine).unwrap().screens[0].definition,
        original_screen
    );
}

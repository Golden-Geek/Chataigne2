use super::*;
use crate::{
    app::{AppEngine, AppNode},
    media::{
        protocol::*,
        tests::{apply, create, fixture, invoke},
    },
};
use golden_core::{
    app::{from_sparse_project_json, to_sparse_project_json_pretty},
    application::{apply_project_intent, service_script_app_operations},
    edit::Edit,
    node::{Node, NodeId, NodeReference, NodeUuid},
    parameter::{ParamValue, ParameterEventBehaviour},
    script::ScriptMethodCall,
    ui_sync::UiEditIntent,
};

mod applications;
mod queries;
mod unavailable;

fn command(engine: &mut AppEngine, kind: MediaCommandKind) -> NodeUuid {
    let MediaEditResult::CommandCreated { command } = apply(
        engine,
        MediaEdit::CreateCommand {
            command_kind: kind,
            label: None,
        },
    ) else {
        panic!()
    };
    command
}
fn parameter(engine: &AppEngine, command: NodeUuid, field: &str) -> NodeId {
    let id = engine.node_id_by_uuid(command).unwrap();
    let folder = engine.nodes.get(id).unwrap().node_data().first_child.unwrap();
    let decl = format!("parameters/{field}");
    engine
        .nodes
        .iter()
        .find_map(|(id, node)| {
            (node.node_data().parent == Some(folder) && node.node_data().meta.decl_id.0 == decl).then_some(id)
        })
        .unwrap()
}
fn set(engine: &mut AppEngine, command: NodeUuid, field: &str, value: ParamValue) {
    let node = parameter(engine, command, field);
    let ack = apply_project_intent(
        engine,
        UiEditIntent::SetParam {
            node,
            value,
            behaviour: ParameterEventBehaviour::Append,
        },
    );
    assert!(ack.success, "{field}: {ack:?}");
}
fn reference(uuid: NodeUuid) -> ParamValue {
    ParamValue::Reference(NodeReference::new(uuid))
}
fn inspect(engine: &mut AppEngine) -> MediaManagerSnapshot {
    let MediaEditResult::Manager { snapshot } = apply(engine, MediaEdit::InspectManager) else {
        panic!()
    };
    snapshot
}

#[test]
fn all_command_kinds_are_typed_passive_and_execute_source_intents_with_atomic_history() {
    for kind in MediaCommandKind::ALL {
        let mut engine = fixture();
        let solid = create(&mut engine, MediaKind::SolidColor);
        let picture = create(&mut engine, MediaKind::Picture);
        let shared = create(&mut engine, MediaKind::SharedTexture);
        let MediaEditResult::Created { reference: peer } = apply(
            &mut engine,
            MediaEdit::AddChannel {
                media: solid.media,
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
        let MediaEditResult::AssetCreated { asset } = apply(
            &mut engine,
            MediaEdit::ImportAsset {
                locator: AssetLocator::ProjectRelative {
                    path: "original.png".to_owned().try_into().unwrap(),
                },
                label: None,
            },
        ) else {
            panic!()
        };
        let before_creation = inspect(&mut engine);
        let command = command(&mut engine, *kind);
        assert_eq!(inspect(&mut engine), before_creation, "creation must not fire {kind:?}");
        let target = match kind {
            MediaCommandKind::DropMatrixMedia
            | MediaCommandKind::SetApplicationFile
            | MediaCommandKind::InspectApplication
            | MediaCommandKind::LaunchMatrixColumn
            | MediaCommandKind::LaunchMatrixCell
            | MediaCommandKind::UnloadMatrix => create(&mut engine, MediaKind::Matrix),
            MediaCommandKind::DropCompositionMedia => create(&mut engine, MediaKind::Composition),
            MediaCommandKind::Picture | MediaCommandKind::ConfigureFitMode => picture,
            MediaCommandKind::SharedTexture => shared,
            _ => solid,
        };
        if matches!(
            kind,
            MediaCommandKind::DropMatrixMedia
                | MediaCommandKind::SetApplicationFile
                | MediaCommandKind::InspectApplication
                | MediaCommandKind::LaunchMatrixColumn
                | MediaCommandKind::LaunchMatrixCell
                | MediaCommandKind::UnloadMatrix
        ) {
            use crate::matrix::protocol::*;
            let MediaEditResult::MatrixCreated { items, .. } = apply(
                &mut engine,
                MediaEdit::Matrix {
                    media: target.media,
                    edit: MatrixEdit::AddLayers { parent: None, count: 1 },
                },
            ) else {
                panic!()
            };
            let MediaEditResult::MatrixCreated { columns, .. } = apply(
                &mut engine,
                MediaEdit::Matrix {
                    media: target.media,
                    edit: MatrixEdit::AddColumns { count: 1 },
                },
            ) else {
                panic!()
            };
            set(
                &mut engine,
                command,
                "matrix_layer",
                reference(match items[0] {
                    MatrixItemId::Layer { id } => id.0,
                    _ => panic!(),
                }),
            );
            set(&mut engine, command, "matrix_column", reference(columns[0].0));
            set(&mut engine, command, "application_owner", reference(columns[0].0));
        }
        for (name, value) in [
            ("label", ParamValue::Str("Program output".into())),
            ("target_folder", reference(folder)),
            ("reference_target", reference(solid.media.0)),
            ("entry", reference(solid.media.0)),
            ("asset", reference(NodeUuid(asset.get()))),
            ("after_channel", reference(peer.channel.0)),
            ("direction", ParamValue::Enum("Later".into())),
            ("resolution_enabled", ParamValue::Bool(true)),
            ("resolution", ParamValue::Vec2(17.0, 9.0)),
            ("fit_mode", ParamValue::Enum("Fill".into())),
            ("rgba", ParamValue::Color(1.0, 0.0, 0.0, 0.5)),
            ("sender", ParamValue::Str("Exact sender".into())),
            ("project_path", ParamValue::File("command.png".into())),
        ] {
            set(&mut engine, command, name, value);
        }
        if *kind != MediaCommandKind::ImportPicture {
            let owner = if *kind == MediaCommandKind::ConfigureReference {
                let MediaEditResult::ReferenceCreated { media } = apply(
                    &mut engine,
                    MediaEdit::CreateReference {
                        target: None,
                        label: None,
                    },
                ) else {
                    panic!()
                };
                media
            } else {
                target.media
            };
            set(&mut engine, command, "media", reference(owner.0));
            set(&mut engine, command, "channel", reference(target.channel.0));
        }
        let id = engine.node_id_by_uuid(command).unwrap();
        let AppNode::MediaCommand(node) = engine.nodes.get(id).unwrap() else {
            panic!()
        };
        assert_eq!(node.execution_rule(), golden_core::engine::NodeExecutionRule::passive());
        assert!(engine.catalog_creatable_items(id).is_empty());
        for field in kind.fields() {
            parameter(&engine, command, field);
        }
        let before = to_sparse_project_json_pretty(&engine).unwrap();
        let history = engine.undo_len();
        let ack = invoke(&mut engine, MediaEdit::ExecuteCommand { command });
        assert!(ack.success, "{kind:?}: {ack:?}");
        let after = to_sparse_project_json_pretty(&engine).unwrap();
        if before != after {
            assert_eq!(engine.undo_len(), history + 1, "{kind:?}");
            assert!(engine.undo().unwrap());
            assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before, "{kind:?} undo");
            assert!(engine.redo().unwrap());
            assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), after, "{kind:?} redo");
        } else {
            assert_eq!(engine.undo_len(), history, "{kind:?}");
        }
        let mut loaded = from_sparse_project_json::<AppNode>(&after).unwrap();
        loaded.apply_edits().unwrap();
        crate::media::validate_project(&loaded).unwrap();
        assert_eq!(inspect(&mut loaded), inspect(&mut engine), "{kind:?} reload");
        let AppNode::MediaCommand(node) = loaded.nodes.get(loaded.node_id_by_uuid(command).unwrap()).unwrap() else {
            panic!()
        };
        assert_eq!(node.execution_rule(), golden_core::engine::NodeExecutionRule::passive());
        assert_eq!(
            inspect(&mut loaded),
            inspect(&mut engine),
            "reload must not fire {kind:?}"
        );
    }
}

#[test]
fn trigger_and_node_method_capture_typed_configuration_and_deliver_committed_callbacks() {
    use golden_core::events::EventKind;
    use golden_core::script::{ScriptNode, ScriptNodeConfig, ScriptSource};
    let mut engine = fixture();
    let command = command(&mut engine, MediaCommandKind::CreateMedia);
    let parent = crate::media::authoring::manager(&engine).unwrap();
    let mut listener = ScriptNode::new("Command results", ScriptNodeConfig { source: ScriptSource::Inline {
        text: "function init(){local.listen({level:2});} function onAppOperationResult(reply){gc.emit('command-method-reply',reply);}".into()
    } });
    listener.budgets.max_wall_time_us_per_load = 1_000_000;
    listener.budgets.max_wall_time_us_per_callback = 1_000_000;
    engine.add_node(listener.into(), Some(parent));
    engine.apply_edits().unwrap();
    set(&mut engine, command, "label", ParamValue::Str("Triggered".into()));
    let history = engine.undo_len();
    set(&mut engine, command, "trigger", ParamValue::Trigger());
    assert_eq!(inspect(&mut engine).media[0].label, "Triggered");
    assert_eq!(engine.undo_len(), history + 1);
    assert!(engine.undo().unwrap());
    assert!(inspect(&mut engine).media.is_empty());
    assert!(engine.redo().unwrap());
    service_script_app_operations(&mut engine).unwrap();
    assert_eq!(inspect(&mut engine).media.len(), 1, "history must not replay a trigger");
    let id = engine.node_id_by_uuid(command).unwrap();
    engine.edits.push(Edit::CallNodeScriptMethod {
        node: id,
        call: ScriptMethodCall::prepare("execute", vec![ParamValue::Str("captured".into())]).unwrap(),
    });
    engine.apply_edits().unwrap();
    engine.edits.push(Edit::SetParam {
        node: parameter(&engine, command, "label"),
        value: ParamValue::Str("Later settings".into()),
        behaviour: ParameterEventBehaviour::Append,
    });
    engine.apply_edits().unwrap();
    service_script_app_operations(&mut engine).unwrap();
    assert!(
        inspect(&mut engine)
            .media
            .iter()
            .any(|media| media.label == "Triggered 2")
    );
    assert!(
        !inspect(&mut engine)
            .media
            .iter()
            .any(|media| media.label == "Later settings")
    );
    let replies = engine
        .ui_event_log()
        .iter()
        .filter_map(|event| match &event.kind {
            EventKind::Custom(event) if event.topic == "command-method-reply" => Some(&*event.payload),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(
        replies
            .iter()
            .any(|reply| reply["request_id"] == "captured" && reply["status"] == "applied")
    );
}

#[test]
fn command_disable_removal_copy_and_schema_rejection_preserve_lifetime_and_configuration() {
    let mut engine = fixture();
    let command = command(&mut engine, MediaCommandKind::CreateMedia);
    set(&mut engine, command, "label", ParamValue::Str("Configured".into()));
    let id = engine.node_id_by_uuid(command).unwrap();
    let call = |engine: &mut AppEngine| {
        engine.edits.push(Edit::CallNodeScriptMethod {
            node: id,
            call: ScriptMethodCall::prepare("execute", vec![ParamValue::Str("discard".into())]).unwrap(),
        });
        engine.apply_edits().unwrap();
    };
    call(&mut engine);
    engine.edits.push(Edit::PatchMeta {
        node: id,
        patch: golden_core::node::NodeMetaPatch {
            enabled: Some(false),
            ..Default::default()
        },
    });
    engine.apply_edits().unwrap();
    service_script_app_operations(&mut engine).unwrap();
    assert!(inspect(&mut engine).media.is_empty());
    assert!(!invoke(&mut engine, MediaEdit::ExecuteCommand { command }).success);
    engine.edits.push(Edit::PatchMeta {
        node: id,
        patch: golden_core::node::NodeMetaPatch {
            enabled: Some(true),
            ..Default::default()
        },
    });
    engine.apply_edits().unwrap();
    call(&mut engine);
    engine.edits.push(Edit::RemoveNode { node: id });
    engine.apply_edits().unwrap();
    service_script_app_operations(&mut engine).unwrap();
    assert!(inspect(&mut engine).media.is_empty());
    assert!(engine.undo().unwrap());
    service_script_app_operations(&mut engine).unwrap();
    assert!(
        inspect(&mut engine).media.is_empty(),
        "restoring a command must not fire it"
    );
    let source = engine.node_id_by_uuid(command).unwrap();
    let parent = crate::media::authoring::manager(&engine).unwrap();
    let ack = apply_project_intent(
        &mut engine,
        UiEditIntent::DuplicateNode {
            source,
            new_parent: parent,
            new_prev_sibling: None,
            initial_params: vec![],
        },
    );
    assert!(ack.success, "{ack:?}");
    let copies = engine
        .nodes
        .iter()
        .filter_map(|(_, node)| matches!(node, AppNode::MediaCommand(_)).then_some(node.node_data().meta.uuid))
        .collect::<Vec<_>>();
    assert_eq!(copies.len(), 2);
    assert_ne!(copies[0], copies[1]);
    for copy in copies {
        let value = engine
            .nodes
            .get(parameter(&engine, copy, "label"))
            .unwrap()
            .engine_param_snapshot()
            .unwrap()
            .value;
        assert_eq!(value, ParamValue::Str("Configured".into()));
    }
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let unsupported = saved.replace(super::NODE_TYPE, "mapgyver.media.command.v999");
    assert!(
        from_sparse_project_json::<AppNode>(&unsupported)
            .err()
            .unwrap()
            .to_string()
            .contains("Unsupported MapGyver command schema")
    );
}

#[test]
fn manager_script_executes_configured_commands_and_receives_committed_results() {
    use golden_core::{
        events::EventKind,
        script::{ScriptNode, ScriptNodeConfig, ScriptSource},
    };
    let mut engine = fixture();
    let command = command(&mut engine, MediaCommandKind::CreateMedia);
    set(&mut engine, command, "label", ParamValue::Str("Script command".into()));
    let parent = crate::media::authoring::manager(&engine).unwrap();
    let source = format!(
        "function init(){{local.listen({{level:1}});local.executeCommand('run','{}');}} function onAppOperationResult(reply){{gc.emit('command-reply',reply);}}",
        command.0
    );
    let mut caller = ScriptNode::new(
        "Command caller",
        ScriptNodeConfig {
            source: ScriptSource::Inline { text: source },
        },
    );
    caller.budgets.max_wall_time_us_per_load = 1_000_000;
    caller.budgets.max_wall_time_us_per_callback = 1_000_000;
    engine.add_node(caller.into(), Some(parent));
    engine.apply_edits().unwrap();
    let history = engine.undo_len();
    service_script_app_operations(&mut engine).unwrap();
    assert_eq!(inspect(&mut engine).media[0].label, "Script command");
    assert_eq!(engine.undo_len(), history + 1);
    let replies = engine
        .ui_event_log()
        .iter()
        .filter_map(|event| match &event.kind {
            EventKind::Custom(event) if event.topic == "command-reply" => Some((*event.payload).clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(replies.len(), 1);
    assert_eq!(replies[0]["request_id"], "run");
    assert_eq!(replies[0]["status"], "applied");
    assert_eq!(replies[0]["value"]["kind"], "Created");
}

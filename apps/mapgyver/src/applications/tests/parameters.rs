use super::*;
use golden_core::{
    edit::Edit,
    node::{Node, NodeId},
    parameter::{ParamValue, ParameterEventBehaviour},
};

fn root(engine: &AppEngine, application: ApplicationId) -> NodeId {
    engine.node_id_by_uuid(application.0).unwrap()
}
fn parameter(engine: &AppEngine, owner: NodeId, name: &str) -> NodeId {
    let folder = engine
        .nodes
        .iter()
        .find(|(_, node)| node.node_data().parent == Some(owner) && crate::parameters::is_parameter_folder(*node))
        .unwrap()
        .0;
    engine
        .nodes
        .iter()
        .find(|(_, node)| {
            node.node_data().parent == Some(folder) && node.node_data().meta.decl_id.0 == format!("parameters/{name}")
        })
        .unwrap()
        .0
}
fn set(engine: &mut AppEngine, node: NodeId, value: ParamValue) -> golden_core::ui_sync::UiAck {
    apply_project_intent(
        engine,
        UiEditIntent::SetParam {
            node,
            value,
            behaviour: ParameterEventBehaviour::Append,
        },
    )
}
fn guard(engine: &AppEngine, application: ApplicationId) -> golden_io::RevisionGuard {
    match engine.nodes.get(root(engine, application)).unwrap() {
        AppNode::Application(node) => node.guard(),
        _ => panic!(),
    }
}
fn children(engine: &AppEngine, application: ApplicationId) -> Vec<NodeId> {
    crate::parameters::domain_children(engine, root(engine, application)).unwrap()
}

#[test]
fn generic_parameters_and_public_configure_share_one_typed_history_and_project_model() {
    let mut engine = fixture();
    let application = create(&mut engine);
    let owner = root(&engine, application);
    assert!(
        engine
            .nodes
            .get(owner)
            .unwrap()
            .project_encode_data()
            .unwrap()
            .is_null()
    );
    let file = parameter(&engine, owner, "app_file");
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    let fence = guard(&engine, application);
    let executable = std::env::current_exe().unwrap().to_string_lossy().into_owned();
    assert!(set(&mut engine, file, ParamValue::File(executable.clone())).success);
    assert!(!fence.is_current());
    assert_eq!(engine.undo_len(), history + 1);
    assert_eq!(
        crate::applications::inspect(&engine, application)
            .unwrap()
            .definition
            .executable,
        Some(executable)
    );
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let loaded = from_sparse_project_json::<AppNode>(&saved).unwrap();
    assert_eq!(
        crate::applications::inspect(&loaded, application).unwrap().definition,
        crate::applications::inspect(&engine, application).unwrap().definition
    );
    assert!(engine.undo().unwrap());
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
    assert!(engine.redo().unwrap());
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), saved);
    let window = parameter(&engine, owner, "window");
    engine.edits.push(Edit::SetParam {
        node: window,
        value: ParamValue::Enum("Normal".into()),
        behaviour: ParameterEventBehaviour::Append,
    });
    engine.apply_edits().unwrap();
    engine.run_tick(std::time::Duration::ZERO).unwrap();
    assert_eq!(
        crate::applications::inspect(&engine, application)
            .unwrap()
            .definition
            .window,
        ApplicationWindow::Normal
    );
}

#[test]
fn configuration_reconciles_real_ordered_entries_and_entry_writes_invalidate_runtime_captures() {
    let mut engine = fixture();
    let application = create(&mut engine);
    let mut definition = ApplicationDefinition {
        arguments: vec!["a".into(), "b".into()],
        environment: Some(vec![("FIRST".into(), "1".into()), ("SECOND".into(), "2".into())]),
        ..Default::default()
    };
    app_edit(
        &mut engine,
        ApplicationEdit::Configure {
            application,
            definition: definition.clone(),
        },
    );
    let ids = children(&engine, application);
    assert_eq!(ids.len(), 4);
    for id in &ids {
        assert!(engine.nodes.get(*id).unwrap().project_encode_data().unwrap().is_null());
    }
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    let fence = guard(&engine, application);
    app_edit(
        &mut engine,
        ApplicationEdit::Configure {
            application,
            definition: definition.clone(),
        },
    );
    assert_eq!(children(&engine, application), ids);
    assert_eq!(engine.undo_len(), history);
    assert!(fence.is_current());
    let argument = parameter(&engine, ids[0], "value");
    assert!(set(&mut engine, argument, ParamValue::Str("literal & --value".into())).success);
    assert!(!fence.is_current());
    definition.arguments[0] = "literal & --value".into();
    assert_eq!(
        crate::applications::inspect(&engine, application).unwrap().definition,
        definition
    );
    let env_value = parameter(&engine, ids[2], "value");
    let fence = guard(&engine, application);
    assert!(set(&mut engine, env_value, ParamValue::Str("updated".into())).success);
    assert!(!fence.is_current());
    definition.environment.as_mut().unwrap()[0].1 = "updated".into();
    assert_eq!(
        crate::applications::inspect(&engine, application).unwrap().definition,
        definition
    );
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let loaded = from_sparse_project_json::<AppNode>(&saved).unwrap();
    assert_eq!(
        crate::applications::inspect(&loaded, application).unwrap().definition,
        definition
    );
    assert!(engine.undo().unwrap());
    assert!(engine.undo().unwrap());
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
    definition.arguments.truncate(1);
    definition.environment = Some(vec![]);
    app_edit(
        &mut engine,
        ApplicationEdit::Configure {
            application,
            definition: definition.clone(),
        },
    );
    assert_eq!(children(&engine, application), vec![ids[0]]);
    assert_eq!(
        crate::applications::inspect(&engine, application).unwrap().definition,
        definition
    );
    definition.environment = None;
    app_edit(
        &mut engine,
        ApplicationEdit::Configure {
            application,
            definition: definition.clone(),
        },
    );
    assert_eq!(
        crate::applications::inspect(&engine, application).unwrap().definition,
        definition
    );
}

#[test]
fn direct_parameter_writes_validate_deadlines_paths_and_environment_without_authored_changes() {
    let mut engine = fixture();
    let application = create(&mut engine);
    let owner = root(&engine, application);
    app_edit(
        &mut engine,
        ApplicationEdit::Configure {
            application,
            definition: ApplicationDefinition {
                environment: Some(vec![("VALID".into(), "ok".into())]),
                ..Default::default()
            },
        },
    );
    let env = children(&engine, application)[0];
    let authored = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    for (node, value) in [
        (
            parameter(&engine, owner, "app_file"),
            ParamValue::File("relative.exe".into()),
        ),
        (parameter(&engine, owner, "initial_ms"), ParamValue::Float(50000.0)),
        (parameter(&engine, owner, "retirement_ms"), ParamValue::Float(0.0)),
        (parameter(&engine, owner, "timeout_ms"), ParamValue::Float(1.5)),
        (parameter(&engine, env, "name"), ParamValue::Str("bad=name".into())),
        (parameter(&engine, env, "value"), ParamValue::Str("bad\0value".into())),
    ] {
        assert!(!set(&mut engine, node, value.clone()).success);
        engine.edits.push(Edit::SetParam {
            node,
            value,
            behaviour: ParameterEventBehaviour::Append,
        });
        assert!(engine.apply_edits().is_err());
        assert_eq!(engine.undo_len(), history);
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), authored);
    }
    // Both fields are validated against the final proposal, independent of write order.
    let definition = ApplicationDefinition {
        restart: ApplicationRestart {
            enabled: true,
            initial_ms: 50000,
            maximum_ms: 60000,
        },
        ..Default::default()
    };
    app_edit(
        &mut engine,
        ApplicationEdit::Configure {
            application,
            definition: definition.clone(),
        },
    );
    assert_eq!(
        crate::applications::inspect(&engine, application).unwrap().definition,
        definition
    );
}

#[test]
fn generic_entry_creation_and_collection_limits_use_the_same_backend_ownership_contract() {
    let mut engine = fixture();
    let application = create(&mut engine);
    let parent = root(&engine, application);
    let fence = guard(&engine, application);
    let ack = apply_project_intent(
        &mut engine,
        UiEditIntent::CreateUserItem {
            parent,
            node_type: super::super::entries::ARGUMENT_TYPE.into(),
            label: None,
            initial_params: vec![],
        },
    );
    assert!(ack.success, "{ack:?}");
    assert!(!fence.is_current());
    assert_eq!(
        crate::applications::inspect(&engine, application)
            .unwrap()
            .definition
            .arguments,
        vec![String::new()]
    );
    let definition = ApplicationDefinition {
        arguments: vec!["x".into(); 256],
        environment: Some((0..128).map(|index| (format!("VAR_{index}"), "value".into())).collect()),
        ..Default::default()
    };
    app_edit(
        &mut engine,
        ApplicationEdit::Configure {
            application,
            definition: definition.clone(),
        },
    );
    assert_eq!(
        crate::applications::inspect(&engine, application).unwrap().definition,
        definition
    );
    let authored = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    let ack = apply_project_intent(
        &mut engine,
        UiEditIntent::CreateUserItem {
            parent,
            node_type: super::super::entries::ARGUMENT_TYPE.into(),
            label: None,
            initial_params: vec![],
        },
    );
    assert!(!ack.success, "{ack:?}");
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), authored);
    assert_eq!(engine.undo_len(), history);
}

#[test]
fn obsolete_application_records_are_rejected_before_recoverable_decode_can_drop_settings() {
    let mut engine = fixture();
    create(&mut engine);
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    assert!(saved.contains("mapgyver.app_control.v1"));
    let obsolete = saved.replace("mapgyver.app_control.v1", "mapgyver.application");
    assert!(from_sparse_project_json::<AppNode>(&obsolete).is_err());
}

#[test]
fn aggregate_size_validation_observes_the_final_atomic_entry_proposal() {
    let mut engine = fixture();
    let application = create(&mut engine);
    let mut definition = ApplicationDefinition {
        arguments: vec!["a".repeat(20000), "b".repeat(5000)],
        ..Default::default()
    };
    app_edit(
        &mut engine,
        ApplicationEdit::Configure {
            application,
            definition: definition.clone(),
        },
    );
    let ids = children(&engine, application);
    let second = parameter(&engine, ids[1], "value");
    let authored = to_sparse_project_json_pretty(&engine).unwrap();
    assert!(!set(&mut engine, second, ParamValue::Str("b".repeat(20000))).success);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), authored);
    definition.arguments.swap(0, 1);
    app_edit(
        &mut engine,
        ApplicationEdit::Configure {
            application,
            definition: definition.clone(),
        },
    );
    assert_eq!(children(&engine, application), ids);
    assert_eq!(
        crate::applications::inspect(&engine, application).unwrap().definition,
        definition
    );
}

#[test]
fn generic_duplication_copies_typed_children_with_fresh_identities_and_one_history_step() {
    let mut engine = fixture();
    let application = create(&mut engine);
    let definition = ApplicationDefinition {
        arguments: vec!["literal & value".into()],
        environment: Some(vec![("EXACT".into(), "value".into())]),
        window: ApplicationWindow::Normal,
        ..Default::default()
    };
    app_edit(
        &mut engine,
        ApplicationEdit::Configure {
            application,
            definition: definition.clone(),
        },
    );
    let source = root(&engine, application);
    let parent = engine.nodes.get(source).unwrap().node_data().parent.unwrap();
    let history = engine.undo_len();
    let ack = apply_project_intent(
        &mut engine,
        UiEditIntent::DuplicateNode {
            source,
            new_parent: parent,
            new_prev_sibling: Some(source),
            initial_params: vec![],
        },
    );
    assert!(ack.success, "{ack:?}");
    assert_eq!(engine.undo_len(), history + 1);
    let copy = engine
        .nodes
        .iter()
        .find_map(|(_, node)| match node {
            AppNode::Application(node) if node.node_data().meta.uuid != application.0 => {
                Some(ApplicationId(node.node_data().meta.uuid))
            }
            _ => None,
        })
        .unwrap();
    assert_eq!(
        crate::applications::inspect(&engine, copy).unwrap().definition,
        definition
    );
    let original = children(&engine, application)
        .into_iter()
        .map(|id| engine.nodes.get(id).unwrap().node_data().meta.uuid)
        .collect::<Vec<_>>();
    for id in children(&engine, copy) {
        assert!(!original.contains(&engine.nodes.get(id).unwrap().node_data().meta.uuid));
    }
    assert!(engine.undo().unwrap());
    assert!(crate::applications::inspect(&engine, copy).is_err());
    assert!(engine.redo().unwrap());
    assert_eq!(
        crate::applications::inspect(&engine, copy).unwrap().definition,
        definition
    );
}

use super::*;
mod pages;
mod scale;
use crate::media::tests::{apply, create, fixture};
use golden_core::{
    app::{from_sparse_project_json, to_sparse_project_json_pretty},
    application::apply_project_intent,
    parameter::{ParamValue, ParameterEventBehaviour},
    ui_sync::UiEditIntent,
};

fn reference(engine: &mut AppEngine, target: Option<MediaId>) -> MediaId {
    let MediaEditResult::ReferenceCreated { media } = apply(engine, MediaEdit::CreateReference { target, label: None })
    else {
        panic!()
    };
    media
}
#[test]
fn references_keep_one_own_output_and_pin_the_selected_source_through_reorder_and_reload() {
    let mut engine = fixture();
    let target = create(&mut engine, MediaKind::SolidColor);
    let MediaEditResult::ReferenceCreated { media } = apply(
        &mut engine,
        MediaEdit::CreateReference {
            target: Some(target.media),
            label: None,
        },
    ) else {
        panic!()
    };
    let output = crate::media::resolve_source_node(&engine, media.0).unwrap();
    let snapshot = channel(&engine, output).unwrap();
    assert_ne!(output.channel, target.channel);
    assert!(
        matches!(snapshot.definition.source, ChannelSource::Reference { target: Some(selected) } if selected == target)
    );
    let MediaEditResult::Created { reference: second } = apply(
        &mut engine,
        MediaEdit::AddChannel {
            media: target.media,
            label: None,
        },
    ) else {
        panic!()
    };
    apply(
        &mut engine,
        MediaEdit::ReorderChannel {
            reference: second,
            after: None,
        },
    );
    let mut resolver = Resolver::new(&engine);
    assert_eq!(
        resolver
            .channels(media)
            .unwrap()
            .map(|node| ChannelId(node.node_data().meta.uuid))
            .collect::<Vec<_>>(),
        vec![output.channel]
    );
    assert!(
        matches!(channel(&engine, output).unwrap().definition.source, ChannelSource::Reference { target: Some(selected) } if selected == target)
    );
    apply(
        &mut engine,
        MediaEdit::SelectReferenceChannel {
            media,
            channel: Some(second.channel),
        },
    );
    assert!(
        matches!(channel(&engine, output).unwrap().definition.source, ChannelSource::Reference { target: Some(selected) } if selected == second)
    );
    apply(&mut engine, MediaEdit::RemoveMedia { media: target.media });
    assert_eq!(Resolver::new(&engine).channels(media).unwrap().count(), 1);
    assert!(engine.undo().unwrap());
    assert_eq!(Resolver::new(&engine).channels(media).unwrap().count(), 1);
    let id = engine.node_id_by_uuid(media.0).unwrap();
    assert_eq!(crate::parameters::domain_children(&engine, id).unwrap().len(), 1);
    let saved = golden_core::app::to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded = golden_core::app::from_sparse_project_json::<AppNode>(&saved).unwrap();
    loaded.apply_edits().unwrap();
    crate::media::validate_project(&loaded).unwrap();
    assert_eq!(Resolver::new(&loaded).channels(media).unwrap().count(), 1);
    assert_eq!(crate::media::resolve_source_node(&loaded, media.0).unwrap(), output);
}

#[test]
fn reference_target_cycles_reject_atomically_for_intents_and_generic_parameters() {
    let mut engine = fixture();
    let solid = create(&mut engine, MediaKind::SolidColor);
    let a = reference(&mut engine, Some(solid.media));
    let b = reference(&mut engine, Some(a));
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    for target in [a, b] {
        let ack = crate::media::tests::invoke(
            &mut engine,
            MediaEdit::ConfigureReference {
                media: a,
                target: Some(target),
            },
        );
        assert!(!ack.success, "cycle must reject: {ack:?}");
        assert_eq!(engine.undo_len(), history);
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
    }
    let node = engine.node_id_by_uuid(a.0).unwrap();
    let AppNode::ReferenceMedia(node) = engine.nodes.get(node).unwrap() else {
        panic!()
    };
    let parameter = node.target_parameter();
    let ack = apply_project_intent(
        &mut engine,
        UiEditIntent::SetParam {
            node: parameter,
            value: ParamValue::Reference(golden_core::node::NodeReference::new(b.0)),
            behaviour: ParameterEventBehaviour::Append,
        },
    );
    assert!(!ack.success);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
}

#[test]
fn missing_target_capture_and_disabled_reference_guards_reject_stale_plans() {
    use golden_core::edit::Edit;
    let mut engine = fixture();
    let source = create(&mut engine, MediaKind::SolidColor);
    let reference = reference(&mut engine, Some(source.media));
    let key = crate::media::resolve_source_node(&engine, reference.0).unwrap();
    let captured = crate::media::capture_channels(&engine, &[key], 8).unwrap();
    apply(&mut engine, MediaEdit::RemoveMedia { media: source.media });
    assert!(!captured.is_current());
    let missing = crate::media::capture_channels(&engine, &[key], 8).unwrap();
    assert!(engine.undo().unwrap());
    assert!(!missing.is_current());
    let enabled = crate::media::capture_channels(&engine, &[key], 8).unwrap();
    let node = engine.node_id_by_uuid(reference.0).unwrap();
    engine.edits.push(Edit::PatchMeta {
        node,
        patch: golden_core::node::NodeMetaPatch {
            enabled: Some(false),
            ..Default::default()
        },
    });
    engine.apply_edits().unwrap();
    assert!(!enabled.is_current());
}

#[test]
fn reference_script_functions_return_committed_identity_and_chain_target_callbacks() {
    use golden_core::{
        application::service_script_app_operations,
        events::EventKind,
        script::{ScriptNode, ScriptNodeConfig, ScriptSource},
    };
    let mut engine = fixture();
    let source = create(&mut engine, MediaKind::SolidColor);
    let parent = crate::media::authoring::manager(&engine).unwrap();
    let text = format!(
        "function init(){{local.listen({{level:1}});local.createReference('create','','Script mirror');}} function onAppOperationResult(reply){{gc.emit('ref-reply',reply);if(reply.request_id==='create'&&reply.status==='applied')local.configureReference('target',reply.value.media,'{}');}}",
        source.media.0.0
    );
    let mut caller = ScriptNode::new(
        "Reference caller",
        ScriptNodeConfig {
            source: ScriptSource::Inline { text },
        },
    );
    caller.budgets.max_wall_time_us_per_load = 1_000_000;
    caller.budgets.max_wall_time_us_per_callback = 1_000_000;
    engine.add_node(caller.into(), Some(parent));
    engine.apply_edits().unwrap();
    let before = engine.undo_len();
    service_script_app_operations(&mut engine).unwrap();
    service_script_app_operations(&mut engine).unwrap();
    let replies = engine
        .ui_event_log()
        .iter()
        .filter_map(|event| match &event.kind {
            EventKind::Custom(event) if event.topic == "ref-reply" => Some((*event.payload).clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(replies.len(), 2);
    assert!(replies.iter().all(|reply| reply["status"] == "applied"));
    let MediaEditResult::ReferenceCreated { media } = serde_json::from_value(replies[0]["value"].clone()).unwrap()
    else {
        panic!()
    };
    assert_eq!(Resolver::new(&engine).channels(media).unwrap().count(), 1);
    assert_eq!(engine.undo_len(), before + 2);
    assert!(engine.undo().unwrap());
    assert_eq!(Resolver::new(&engine).channels(media).unwrap().count(), 1);
    assert!(matches!(
        Resolver::new(&engine).target(media).unwrap(),
        Target::Unconfigured
    ));
}

#[test]
fn mixed_composition_feedback_through_reference_aliases_is_rejected() {
    use crate::media::composition::protocol::CompositionEdit;
    let mut engine = fixture();
    let composition = create(&mut engine, MediaKind::Composition);
    let a = reference(&mut engine, Some(composition.media));
    let b = reference(&mut engine, Some(a));
    let alias = crate::media::resolve_source_node(&engine, b.0).unwrap();
    assert!(channel(&engine, alias).is_ok());
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let ack = crate::media::tests::invoke(
        &mut engine,
        MediaEdit::Composition {
            reference: composition,
            edit: CompositionEdit::AddInstances { sources: vec![alias] },
        },
    );
    assert!(!ack.success, "mixed cycle must reject: {ack:?}");
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
}

#[test]
fn reference_retarget_detects_feedback_from_existing_missing_channel_aliases() {
    use crate::media::composition::{children, protocol::InstanceDefinition};
    use golden_core::edit::Edit;
    let mut engine = fixture();
    let composition = create(&mut engine, MediaKind::Composition);
    let a = reference(&mut engine, None);
    let output = crate::media::resolve_source_node(&engine, a.0).unwrap();
    let parent = engine.node_id_by_uuid(composition.media.0).unwrap();
    engine.edits.push(Edit::AddNodeTree {
        parent,
        prev_sibling: None,
        tree: children::reference_tree(
            "Missing alias".into(),
            Some(crate::media::MediaSource {
                media: a,
                channel: Some(output.channel),
            }),
            InstanceDefinition {
                source: Some(ChannelReference {
                    media: a,
                    channel: output.channel,
                }),
                ..Default::default()
            },
        )
        .0,
    });
    engine.apply_edits().unwrap();
    crate::media::validate_project(&engine).unwrap();
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let ack = crate::media::tests::invoke(
        &mut engine,
        MediaEdit::ConfigureReference {
            media: a,
            target: Some(composition.media),
        },
    );
    assert!(!ack.success, "retarget closes the mixed cycle: {ack:?}");
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
    let loaded = from_sparse_project_json::<AppNode>(&before).unwrap();
    crate::media::validate_project(&loaded).unwrap();
}

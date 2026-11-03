use super::*;
use golden_core::node::Node;

fn drop(engine: &mut AppEngine, owner: ChannelReference, source: ChannelReference, copy: bool) -> InstanceReference {
    let MediaEditResult::InstancesCreated { instances } = edit(
        engine,
        owner,
        CompositionEdit::DropSource {
            source: source.media.0,
            instance: None,
            copy,
            position: Point { x: 12.0, y: 34.0 },
        },
    ) else {
        panic!()
    };
    instances[0]
}

#[test]
fn default_drop_owns_a_reference_and_alt_drop_copies_all_variants_independently() {
    let mut engine = fixture();
    let source = create(&mut engine, MediaKind::SolidColor);
    apply(
        &mut engine,
        MediaEdit::AddChannel {
            media: source.media,
            label: Some("Second variant".into()),
        },
    );
    let owner = create(&mut engine, MediaKind::Composition);
    let history = engine.undo_len();
    let alias = drop(&mut engine, owner, source, false);
    let copied = drop(&mut engine, owner, source, true);
    assert_eq!(engine.undo_len(), history + 2);
    let parent = engine.node_id_by_uuid(owner.media.0).unwrap();
    let reference = engine.node_id_by_uuid(alias.instance.0).unwrap();
    let copy = engine.node_id_by_uuid(copied.instance.0).unwrap();
    assert!(matches!(engine.nodes.get(reference), Some(AppNode::ReferenceMedia(_))));
    assert!(matches!(engine.nodes.get(copy), Some(AppNode::Media(_))));
    assert_eq!(engine.nodes.get(reference).unwrap().node_data().parent, Some(parent));
    assert_eq!(engine.nodes.get(copy).unwrap().node_data().parent, Some(parent));
    let snapshot = inspect(&mut engine, owner);
    assert_eq!(snapshot.instances[0].definition.transform.position, [12.0, 34.0]);
    assert_eq!(snapshot.instances[1].definition.transform.position, [12.0, 34.0]);
    let MediaEditResult::Channels { page } = apply(
        &mut engine,
        MediaEdit::InspectChannels {
            query: MediaChannelsQuery {
                media: MediaId(copied.instance.0),
                start: 0,
                count: 32,
            },
        },
    ) else {
        panic!()
    };
    assert_eq!(page.total, 2);
    assert_ne!(page.channels[0].reference.channel, source.channel);
    apply(
        &mut engine,
        MediaEdit::ConfigureChannel {
            reference: page.channels[0].reference,
            source: ChannelSource::SolidColor {
                rgba: [0.0, 0.0, 1.0, 1.0],
            },
        },
    );
    assert_eq!(
        crate::media::channel(&engine, source).unwrap().definition.source,
        ChannelSource::SolidColor { rgba: [1.0; 4] }
    );
    let json = to_sparse_project_json_pretty(&engine).unwrap();
    assert!(!json.contains("mapgyver.composition.instance"));
    let mut loaded = from_sparse_project_json::<AppNode>(&json).unwrap();
    loaded.apply_edits().unwrap();
    assert_eq!(inspect(&mut loaded, owner), inspect(&mut engine, owner));
    assert!(engine.undo().unwrap());
    assert!(engine.undo().unwrap());
    assert_eq!(inspect(&mut engine, owner).instances.len(), 1);
    assert!(engine.redo().unwrap());
    assert_eq!(inspect(&mut engine, owner).instances, snapshot.instances);
}

#[test]
fn nested_copy_remaps_internal_media_targets_and_preserves_external_targets() {
    let mut engine = fixture();
    let source = create(&mut engine, MediaKind::SolidColor);
    let a = create(&mut engine, MediaKind::Composition);
    let b = create(&mut engine, MediaKind::Composition);
    let owned = drop(&mut engine, a, source, true);
    let owned_source = inspect(&mut engine, a).instances[0].definition.source.unwrap();
    drop(&mut engine, a, owned_source, false);
    drop(&mut engine, a, source, false);
    let copied = drop(&mut engine, b, a, true);
    let copied_output = inspect(&mut engine, b).instances[0].definition.source.unwrap();
    let nested = inspect(&mut engine, copied_output);
    let internal = nested.instances[0].reference.instance;
    assert_ne!(internal, owned.instance);
    let read_target = |engine: &AppEngine, media: InstanceId| {
        let AppNode::ReferenceMedia(node) = engine.nodes.get(engine.node_id_by_uuid(media.0).unwrap()).unwrap() else {
            panic!()
        };
        node.target()
    };
    assert_eq!(
        read_target(&engine, nested.instances[1].reference.instance),
        Some(MediaId(internal.0))
    );
    assert_eq!(
        read_target(&engine, nested.instances[2].reference.instance),
        Some(source.media)
    );
    assert_ne!(copied.instance.0, a.media.0);
    crate::media::validate_project(&engine).unwrap();
}

#[test]
fn nested_missing_target_undo_and_child_disabling_invalidate_captured_plans() {
    let mut engine = fixture();
    let source = create(&mut engine, MediaKind::SolidColor);
    let owner = create(&mut engine, MediaKind::Composition);
    let target = drop(&mut engine, owner, source, true);
    let owned_source = inspect(&mut engine, owner).instances[0].definition.source.unwrap();
    let alias = drop(&mut engine, owner, owned_source, false);
    let alias_source = inspect(&mut engine, owner).instances[1].definition.source.unwrap();
    apply(
        &mut engine,
        MediaEdit::RemoveMedia {
            media: MediaId(target.instance.0),
        },
    );
    let missing = crate::media::capture_channels(&engine, &[alias_source], 16).unwrap();
    assert!(engine.undo().unwrap());
    assert!(
        !missing.is_current(),
        "nested target restoration retires the missing capture"
    );
    let captured = crate::media::capture_channels(&engine, &[owner], 16).unwrap();
    engine.edits.push(golden_core::edit::Edit::PatchMeta {
        node: engine.node_id_by_uuid(alias.instance.0).unwrap(),
        patch: golden_core::node::NodeMetaPatch {
            enabled: Some(false),
            ..Default::default()
        },
    });
    engine.apply_edits().unwrap();
    assert!(!captured.is_current());
}

#[test]
fn named_drop_method_delivers_committed_reference_and_independent_copy_callbacks() {
    use golden_core::{
        application::service_script_app_operations,
        events::EventKind,
        script::{ScriptNode, ScriptNodeConfig, ScriptSource},
    };
    let mut engine = fixture();
    let source = create(&mut engine, MediaKind::SolidColor);
    let owner = create(&mut engine, MediaKind::Composition);
    let call = format!(
        "'{}','{}','{}',''",
        owner.media.0.0, owner.channel.0.0, source.media.0.0
    );
    let text = format!(
        "function init(){{local.listen({{level:1}});local.dropCompositionMedia('reference',{call},false,10,20);}}\nfunction onAppOperationResult(reply){{gc.emit('owned-drop-reply',reply);if(reply.request_id==='reference')local.dropCompositionMedia('copy',{call},true,30,40);}}"
    );
    let mut node = ScriptNode::new(
        "Owned media caller",
        ScriptNodeConfig {
            source: ScriptSource::Inline { text },
        },
    );
    node.budgets.max_wall_time_us_per_load = 1_000_000;
    node.budgets.max_wall_time_us_per_callback = 1_000_000;
    let manager = crate::media::authoring::manager(&engine).unwrap();
    engine.add_node(node.into(), Some(manager));
    engine.apply_edits().unwrap();
    let history = engine.undo_len();
    service_script_app_operations(&mut engine).unwrap();
    service_script_app_operations(&mut engine).unwrap();
    let replies = engine
        .ui_event_log()
        .iter()
        .filter_map(|event| match &event.kind {
            EventKind::Custom(event) if event.topic == "owned-drop-reply" => Some((*event.payload).clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(replies.len(), 2);
    assert!(replies.iter().all(|reply| reply["status"] == "applied"), "{replies:?}");
    assert_eq!(engine.undo_len(), history + 2);
    let snapshot = inspect(&mut engine, owner);
    for (index, item) in snapshot.instances.iter().enumerate() {
        assert_eq!(
            replies[index]["value"]["instances"][0]["instance"],
            serde_json::to_value(item.reference.instance).unwrap()
        );
        let node = engine
            .nodes
            .get(engine.node_id_by_uuid(item.reference.instance.0).unwrap())
            .unwrap();
        assert_eq!(
            node.get_type(),
            if index == 0 {
                "mapgyver.media.reference.v1"
            } else {
                "mapgyver.media.v5"
            }
        );
    }
}

#[test]
fn replacing_an_owned_copy_preserves_exact_placement_and_one_atomic_undo() {
    use golden_core::{
        edit::Edit,
        parameter::{ParamValue, ParameterEventBehaviour},
    };
    let mut engine = fixture();
    let source = create(&mut engine, MediaKind::SolidColor);
    let picture = create(&mut engine, MediaKind::Picture);
    let owner = create(&mut engine, MediaKind::Composition);
    let copied = drop(&mut engine, owner, source, true);
    let id = engine.node_id_by_uuid(copied.instance.0).unwrap();
    let position = super::super::children::placement(&engine, id)
        .unwrap()
        .position_parameter();
    let exact = ParamValue::Vec2(12.123456789123, 34.987654321987);
    engine.edits.push(Edit::SetParam {
        node: position,
        value: exact.clone(),
        behaviour: ParameterEventBehaviour::Append,
    });
    engine.apply_edits().unwrap();
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    let MediaEditResult::InstanceChanged { snapshot } = edit(
        &mut engine,
        owner,
        CompositionEdit::DropSource {
            source: picture.media.0,
            instance: Some(copied.instance),
            copy: false,
            position: Point { x: 900.0, y: 800.0 },
        },
    ) else {
        panic!()
    };
    assert_ne!(snapshot.reference.instance, copied.instance);
    assert!(engine.node_id_by_uuid(copied.instance.0).is_none());
    assert_eq!(
        engine
            .nodes
            .get(position)
            .unwrap()
            .engine_param_snapshot()
            .unwrap()
            .value,
        exact
    );
    assert_eq!(engine.undo_len(), history + 1);
    assert!(engine.undo().unwrap());
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
    assert!(engine.redo().unwrap());
    assert_eq!(inspect(&mut engine, owner).instances[0], snapshot);
}

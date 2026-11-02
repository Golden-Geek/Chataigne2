use super::*;
use golden_core::{
    edit::{Edit, NodeTree},
    node::NodeUuid,
};

fn page(engine: &mut AppEngine, media: MediaId, start: u32, count: u16) -> MediaChannelsPage {
    let MediaEditResult::Channels { page } = apply(
        engine,
        MediaEdit::InspectChannels {
            query: MediaChannelsQuery { media, start, count },
        },
    ) else {
        panic!()
    };
    page
}

#[test]
fn pages_bound_projection_without_copied_channel_nodes_and_preserve_history() {
    let mut engine = fixture();
    let definition = ChannelDefinition::for_media(MediaKind::SolidColor);
    let (mut tree, target) = crate::media::authoring::media_tree("Many variants".into(), definition.clone());
    let mut ids = vec![target.channel];
    for index in 1..512 {
        let node = crate::media::ChannelNode::from_definition(format!("Variant {index}"), definition.clone());
        ids.push(ChannelId(node.node_data().meta.uuid));
        tree = tree.with_child(NodeTree::new(node).as_user_item());
    }
    let manager = crate::media::authoring::manager(&engine).unwrap();
    let mut previous = target.media;
    let mut references = Vec::new();
    let mut trees = vec![tree];
    for index in 0..128 {
        let node = ReferenceNode::from_target(format!("Mirror {index}"), Some(previous));
        previous = MediaId(node.node_data().meta.uuid);
        references.push(previous);
        trees.push(NodeTree::new(node));
    }
    // One detached subtree installs all known channels and References without repeated mutations.
    let folder = crate::media::MediaFolderNode::new("Bounded read fixture".into());
    let tree = trees.into_iter().fold(NodeTree::new(folder), |tree, child| {
        tree.with_child(child.as_user_item())
    });
    engine.edits.push(Edit::AddNodeTree {
        parent: manager,
        prev_sibling: None,
        tree,
    });
    engine.apply_edits().unwrap();
    crate::media::validate_project(&engine).unwrap();
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    let nodes = engine.nodes.len();
    for media in references {
        let read = page(&mut engine, media, 480, 32);
        assert_eq!(read.status, MediaChannelsStatus::Ready);
        assert_eq!(read.total, 512);
        assert_eq!(read.channels.len(), 32);
        assert_eq!(
            read.channels.iter().map(|c| c.reference.channel).collect::<Vec<_>>(),
            ids[480..]
        );
        assert!(read.channels.iter().all(|c| c.reference.media == media));
    }
    assert_eq!(page(&mut engine, previous, u32::MAX, 1).channels.len(), 0);
    for count in [0, 129, u16::MAX] {
        assert!(
            !crate::media::tests::invoke(
                &mut engine,
                MediaEdit::InspectChannels {
                    query: MediaChannelsQuery {
                        media: previous,
                        start: 0,
                        count
                    }
                }
            )
            .success
        );
    }
    assert_eq!(engine.nodes.len(), nodes);
    assert_eq!(engine.undo_len(), history);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
}

#[test]
fn channel_pages_follow_rename_reorder_remove_retarget_and_project_recovery() {
    let mut engine = fixture();
    let source = create(&mut engine, MediaKind::SolidColor);
    let unbound = reference(&mut engine, None);
    let mirror = reference(&mut engine, Some(unbound));
    assert_eq!(
        page(&mut engine, mirror, 0, 32).status,
        MediaChannelsStatus::Unconfigured
    );
    apply(
        &mut engine,
        MediaEdit::ConfigureReference {
            media: unbound,
            target: Some(source.media),
        },
    );
    let MediaEditResult::Created { reference: peer } = apply(
        &mut engine,
        MediaEdit::AddChannel {
            media: source.media,
            label: Some("Peer".into()),
        },
    ) else {
        panic!()
    };
    apply(
        &mut engine,
        MediaEdit::RenameChannel {
            reference: source,
            label: "Renamed".into(),
        },
    );
    apply(
        &mut engine,
        MediaEdit::ReorderChannel {
            reference: peer,
            after: None,
        },
    );
    let expected = page(&mut engine, mirror, 0, 32);
    assert_eq!(
        expected.channels.iter().map(|c| c.label.as_str()).collect::<Vec<_>>(),
        ["Peer", "Renamed"]
    );
    apply(&mut engine, MediaEdit::RemoveChannel { reference: peer });
    assert_eq!(page(&mut engine, mirror, 0, 32).total, 1);
    assert!(engine.undo().unwrap());
    assert_eq!(page(&mut engine, mirror, 0, 32), expected);
    apply(&mut engine, MediaEdit::RemoveMedia { media: source.media });
    let missing = page(&mut engine, mirror, 0, 32);
    assert_eq!(missing.status, MediaChannelsStatus::MissingTarget);
    assert!(missing.channels.is_empty());
    let json = to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded = from_sparse_project_json::<AppNode>(&json).unwrap();
    loaded.apply_edits().unwrap();
    assert_eq!(page(&mut loaded, mirror, 0, 32), missing);
    assert!(engine.undo().unwrap());
    assert_eq!(page(&mut engine, mirror, 0, 32), expected);
    assert_eq!(
        page(&mut engine, MediaId(NodeUuid(uuid::Uuid::new_v4())), 0, 32).status,
        MediaChannelsStatus::MissingMedia
    );
}

#[test]
fn named_channel_page_method_delivers_read_only_committed_callbacks_and_checks_bounds() {
    use golden_core::{
        application::service_script_app_operations,
        events::EventKind,
        parameter::ParamValue,
        script::{ScriptNode, ScriptNodeConfig, ScriptSource},
    };
    let mut engine = fixture();
    let source = create(&mut engine, MediaKind::SolidColor);
    let mirror = reference(&mut engine, Some(source.media));
    for (start, count) in [(-1.0, 32.0), (0.5, 32.0), (0.0, 0.0), (0.0, 129.0), (0.0, 1.5)] {
        assert!(
            crate::media::source_api::operation(
                "inspectChannels",
                &[
                    ParamValue::Str(mirror.0.0.to_string()),
                    ParamValue::Float(start),
                    ParamValue::Float(count)
                ]
            )
            .is_err()
        );
    }
    let text = format!(
        "function init(){{local.listen({{level:1}});local.inspectChannels('page','{}',0,32);}}\nfunction onAppOperationResult(reply){{gc.emit('channel-page-reply',reply);}}",
        mirror.0.0
    );
    let mut caller = ScriptNode::new(
        "Channel page reader",
        ScriptNodeConfig {
            source: ScriptSource::Inline { text },
        },
    );
    caller.budgets.max_wall_time_us_per_load = 1_000_000;
    caller.budgets.max_wall_time_us_per_callback = 1_000_000;
    let parent = crate::media::authoring::manager(&engine).unwrap();
    engine.add_node(caller.into(), Some(parent));
    engine.apply_edits().unwrap();
    let history = engine.undo_len();
    service_script_app_operations(&mut engine).unwrap();
    let replies = engine
        .ui_event_log()
        .iter()
        .filter_map(|event| match &event.kind {
            EventKind::Custom(event) if event.topic == "channel-page-reply" => Some(&*event.payload),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(replies.len(), 1);
    assert_eq!(replies[0]["status"], "applied");
    let MediaEditResult::Channels { page: read } = serde_json::from_value(replies[0]["value"].clone()).unwrap() else {
        panic!()
    };
    assert_eq!(read.total, 1);
    assert_eq!(
        read.channels[0].reference,
        ChannelReference {
            media: mirror,
            channel: source.channel
        }
    );
    assert_eq!(engine.undo_len(), history);
}

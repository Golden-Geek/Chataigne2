use super::*;
use crate::{app::AppNode, media::sender_catalog::Source};
use golden_core::{
    app::{from_sparse_project_json, to_sparse_project_json_pretty},
    node::Node,
    parameter::ParamValue,
};
use std::time::Duration;

fn publish(engine: &mut AppEngine, kind: MediaKind, names: &[String]) {
    Source::capture_for(engine, kind).unwrap().publish(names, &[]).unwrap();
    engine.run_tick(Duration::from_millis(1)).unwrap();
}

fn source(engine: &AppEngine, reference: ChannelReference) -> ChannelSource {
    crate::media::channel(engine, reference).unwrap().definition.source
}

#[test]
fn ndi_sender_catalog_is_separate_and_exact_selection_survives_loss_mode_switch_and_reload() {
    let mut engine = fixture();
    let ndi_name = format!("Studio ({} live)", "é".repeat(150));
    publish(&mut engine, MediaKind::SharedTexture, &["Existing Spout".into()]);
    let empty = create(&mut engine, MediaKind::Ndi);
    assert_eq!(source(&engine, empty), ChannelSource::Ndi { sender: None });
    publish(&mut engine, MediaKind::Ndi, &[ndi_name.clone(), "Peer (Cam)".into()]);
    let selected = create(&mut engine, MediaKind::Ndi);
    let expected = ChannelSource::Ndi {
        sender: Some(ndi_name.clone()),
    };
    assert_eq!(source(&engine, selected), expected);
    let MediaEditResult::Created { reference: extra } = apply(
        &mut engine,
        MediaEdit::AddChannel {
            media: selected.media,
            label: Some("Another input".into()),
        },
    ) else {
        panic!("expected new NDI channel");
    };
    assert_eq!(source(&engine, extra), expected);
    let spout = create(&mut engine, MediaKind::SharedTexture);
    assert_eq!(
        source(&engine, spout),
        ChannelSource::Spout {
            sender: Some("Existing Spout".into())
        }
    );

    let manual = |engine: &mut AppEngine, mode| {
        apply(
            engine,
            MediaEdit::ConfigureMediaSenderSelection {
                media: selected.media,
                mode,
            },
        )
    };
    manual(&mut engine, SenderSelectionMode::Manual);
    let AppNode::MediaChannel(channel) = engine
        .nodes
        .get(engine.node_id_by_uuid(selected.channel.0).unwrap())
        .unwrap()
    else {
        panic!("expected NDI Channel");
    };
    let manual_parameter = engine
        .nodes
        .get(channel.sender_parameter(SenderSelectionMode::Manual))
        .unwrap();
    assert_eq!(manual_parameter.node_data().meta.decl_id.0, "parameters/ndi_sender");
    assert_eq!(
        manual_parameter.engine_param_snapshot().unwrap().value,
        ParamValue::Str(ndi_name.clone())
    );
    assert_eq!(source(&engine, selected), expected);
    publish(&mut engine, MediaKind::Ndi, &["Peer (Cam)".into()]);
    manual(&mut engine, SenderSelectionMode::Auto);
    assert_eq!(source(&engine, selected), expected);
    assert_eq!(source(&engine, empty), ChannelSource::Ndi { sender: None });
    assert!(engine.undo().unwrap());
    engine.settle_edit_notifications(16).unwrap();
    assert_eq!(source(&engine, selected), expected);

    let json = to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded = from_sparse_project_json::<AppNode>(&json).unwrap();
    loaded.apply_edits().unwrap();
    assert_eq!(source(&loaded, selected), expected);
    assert_eq!(source(&loaded, extra), expected);
    assert_eq!(source(&loaded, spout), source(&engine, spout));
}

#[test]
fn additional_ndi_route_is_typed_and_survives_removal_undo_and_reload() {
    use crate::audio::{ndi_route, voice};
    use golden_core::parameter::ParameterEventBehaviour;

    let mut engine = fixture();
    let reference = create(&mut engine, MediaKind::Ndi);
    let parent = engine.node_id_by_uuid(reference.channel.0).unwrap();
    let video = create(&mut engine, MediaKind::Video);
    let video_channel = engine.node_id_by_uuid(video.channel.0).unwrap();
    let add = |engine: &mut AppEngine, parent| {
        apply_project_intent(
            engine,
            UiEditIntent::CreateUserItem {
                parent,
                node_type: ndi_route::TYPE.into(),
                label: None,
                initial_params: vec![],
            },
        )
    };
    assert!(!add(&mut engine, video_channel).success);
    let created = add(&mut engine, parent).created_node.expect("created audio route");
    let parameter = |engine: &AppEngine, route, name: &str| {
        engine
            .process_tree_snapshot()
            .child_ids(route)
            .into_iter()
            .find(|id| engine.nodes.get(*id).unwrap().node_data().meta.decl_id.0 == name)
            .unwrap()
    };
    let set = |engine: &mut AppEngine, node, value| {
        apply_project_intent(
            engine,
            UiEditIntent::SetParam {
                node,
                value,
                behaviour: ParameterEventBehaviour::Append,
            },
        )
        .success
    };
    let input = parameter(&engine, created.node_id, "source_channel");
    let output = parameter(&engine, created.node_id, "destination");
    crate::tests::clamps_and_undo(&mut engine, input, ParamValue::Int(2), ParamValue::Int(3));
    crate::tests::clamps_and_undo(&mut engine, input, ParamValue::Int(257), ParamValue::Int(256));
    assert!(set(&mut engine, input, ParamValue::Int(4)));
    assert!(!set(&mut engine, output, ParamValue::Enum("invalid".into())));
    assert!(set(&mut engine, output, ParamValue::Enum(voice::RIGHT.into())));
    assert!(apply_project_intent(&mut engine, UiEditIntent::RemoveNode { node: created.node_id }).success);
    assert!(engine.node_id_by_uuid(created.uuid).is_none());
    assert!(engine.undo().unwrap());
    engine.settle_edit_notifications(16).unwrap();

    let route_values = |engine: &AppEngine| {
        let route = engine.node_id_by_uuid(created.uuid).expect("saved audio route");
        ["source_channel", "destination"].map(|name| {
            engine
                .nodes
                .get(parameter(engine, route, name))
                .unwrap()
                .engine_param_snapshot()
                .unwrap()
                .value
        })
    };
    let expected = [ParamValue::Int(4), ParamValue::Enum(voice::RIGHT.into())];
    assert_eq!(route_values(&engine), expected);
    let json = to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded = from_sparse_project_json::<AppNode>(&json).unwrap();
    loaded.apply_edits().unwrap();
    golden_core::app::prepare_engine_for_runtime(&mut loaded).unwrap();
    crate::media::validate_project(&loaded).unwrap();
    assert_eq!(route_values(&loaded), expected);
}

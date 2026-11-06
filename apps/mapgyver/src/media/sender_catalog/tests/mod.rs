use super::*;
use crate::media::{
    protocol::*,
    tests::{apply, create, fixture},
};
use golden_core::{app::prepare_engine_for_runtime, parameter::ParameterEventBehaviour};
use std::time::Duration;

fn engine() -> (AppEngine, ChannelReference, ChannelReference) {
    let mut engine = fixture();
    let first = create(&mut engine, MediaKind::SharedTexture);
    let MediaEditResult::Created { reference: second } = apply(
        &mut engine,
        MediaEdit::AddChannel {
            media: first.media,
            label: None,
        },
    ) else {
        panic!()
    };
    apply(
        &mut engine,
        MediaEdit::ConfigureChannel {
            reference: first,
            source: ChannelSource::Spout {
                sender: Some("Exact missing α".into()),
            },
        },
    );
    apply(
        &mut engine,
        MediaEdit::ConfigureMediaSenderSelection {
            media: first.media,
            mode: SenderSelectionMode::Auto,
        },
    );
    prepare_engine_for_runtime(&mut engine).unwrap();
    (engine, first, second)
}
fn parameter(engine: &AppEngine, reference: ChannelReference) -> NodeId {
    let Some(AppNode::MediaChannel(channel)) = engine.nodes.get(engine.node_id_by_uuid(reference.channel.0).unwrap())
    else {
        panic!()
    };
    channel.sender_parameter(SenderSelectionMode::Auto)
}
fn drain(engine: &mut AppEngine) {
    engine.run_tick(Duration::from_millis(1)).unwrap();
}
fn available(engine: &AppEngine, reference: ChannelReference) -> Vec<String> {
    engine
        .nodes
        .get(parameter(engine, reference))
        .unwrap()
        .engine_param_snapshot()
        .unwrap()
        .constraints
        .enum_options
        .as_slice()
        .iter()
        .map(|entry| entry.variant_id.clone())
        .collect()
}

#[test]
fn live_catalog_is_shared_bounded_and_deduplicated_without_retaining_historical_names() {
    let names = (0..80).map(|i| format!("Sender {i}")).collect::<Vec<_>>();
    assert_eq!(choices(&names).len(), 65);
    assert_eq!(choices(&["A".into(), "A".into(), "".into(), "bad\0".into()]).len(), 2);
}

#[test]
fn metadata_disappearance_rename_and_return_preserve_exact_choice_history_and_channel_isolation() {
    let (mut engine, first, second) = engine();
    let history = (engine.undo_len(), engine.redo_len());
    let mut source = Source::capture(&engine).unwrap();
    for names in [
        vec!["Exact missing α".into(), "Other".into()],
        vec![],
        vec!["Renamed".into()],
        vec!["Exact missing α".into()],
    ] {
        source.publish(&names).unwrap();
        drain(&mut engine);
        assert_eq!((engine.undo_len(), engine.redo_len()), history);
        assert_eq!(
            engine
                .nodes
                .get(parameter(&engine, first))
                .unwrap()
                .engine_param_snapshot()
                .unwrap()
                .value,
            ParamValue::Enum("Exact missing α".into())
        );
        assert_eq!(
            engine
                .nodes
                .get(parameter(&engine, second))
                .unwrap()
                .engine_param_snapshot()
                .unwrap()
                .value,
            ParamValue::Enum("".into())
        );
        assert_eq!(
            available(&engine, first),
            std::iter::once(String::new()).chain(names).collect::<Vec<_>>()
        );
    }
}

#[test]
fn pending_metadata_is_latest_only_and_cannot_overwrite_new_values_or_retired_projects() {
    let (mut engine, first, second) = engine();
    let mut source = Source::capture(&engine).unwrap();
    source.publish(&["Older".into()]).unwrap();
    let node = parameter(&engine, first);
    engine.edits.push(Edit::SetParam {
        node,
        value: ParamValue::Enum("New exact".into()),
        behaviour: ParameterEventBehaviour::Append,
    });
    engine.apply_edits().unwrap();
    source.refresh(&engine).unwrap();
    source.publish(&["Latest".into()]).unwrap();
    drain(&mut engine);
    assert_eq!(
        engine.nodes.get(node).unwrap().engine_param_snapshot().unwrap().value,
        ParamValue::Enum("New exact".into())
    );
    source.refresh(&engine).unwrap();
    source.publish(&["Latest".into()]).unwrap();
    drain(&mut engine);
    assert_eq!(available(&engine, first), vec!["", "Latest"]);
    assert_eq!(available(&engine, second), vec!["", "Latest"]);
    assert_eq!(
        engine.nodes.get(node).unwrap().engine_param_snapshot().unwrap().value,
        ParamValue::Enum("New exact".into())
    );
    drop(engine);
    assert!(source.publish(&["Retired".into()]).is_err());
}

#[test]
fn refreshing_one_binding_before_delivery_keeps_the_whole_pending_catalog_batch() {
    let (mut engine, first, second) = engine();
    let mut source = Source::capture(&engine).unwrap();
    source.publish(&["Same catalog".into()]).unwrap();
    // Refresh one captured binding from the still-older installed metadata, before delivery.
    let channel = source.selections[0].channel;
    source.selections[0] = Source::selection(&engine, channel).unwrap();
    source.publish(&["Same catalog".into()]).unwrap();
    drain(&mut engine);
    assert_eq!(available(&engine, first), vec!["", "Same catalog"]);
    assert_eq!(available(&engine, second), vec!["", "Same catalog"]);
}

#[test]
fn conditional_enum_domain_is_restored_before_post_load_sender_disappearance() {
    let (mut engine, first, _) = engine();
    Source::capture(&engine)
        .unwrap()
        .publish(&["Exact missing α".into()])
        .unwrap();
    drain(&mut engine);
    let saved = golden_core::app::to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded = golden_core::app::from_sparse_project_json::<AppNode>(&saved).unwrap();
    prepare_engine_for_runtime(&mut loaded).unwrap();
    let Some(AppNode::Media(media)) = loaded.nodes.get(loaded.node_id_by_uuid(first.media.0).unwrap()) else {
        panic!()
    };
    let mode = loaded
        .nodes
        .get(media.sender_selection_parameter().unwrap())
        .unwrap()
        .engine_param_snapshot()
        .unwrap();
    assert!(mode.constraints.enum_options.contains_variant("Manual"));
    assert!(mode.constraints.enum_options.contains_variant("Auto"));
    let state = loaded
        .nodes
        .get(parameter(&loaded, first))
        .unwrap()
        .engine_param_snapshot()
        .unwrap();
    assert_eq!(
        state.constraints.enum_domain,
        Some(golden_core::parameter::ParameterEnumDomainPolicy::Open)
    );
    let mut source = Source::capture(&loaded).unwrap();
    source.publish(&[]).unwrap();
    drain(&mut loaded);
    assert_eq!(
        loaded
            .nodes
            .get(parameter(&loaded, first))
            .unwrap()
            .engine_param_snapshot()
            .unwrap()
            .value,
        ParamValue::Enum("Exact missing α".into())
    );
}

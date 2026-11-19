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

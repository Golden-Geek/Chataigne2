use super::*;
use golden_core::node::NodeMetaPatch;

#[test]
fn video_audio_binding_uses_live_effective_enablement_after_folder_and_parent_toggles() {
    let mut engine = fixture();
    let reference = create(&mut engine, MediaKind::Video);
    let owner = engine.node_id_by_uuid(reference.channel.0).unwrap();
    let snapshot = engine.process_tree_snapshot();
    let parameters = snapshot.find_child_by_decl_id(owner, "parameters").unwrap();
    let folder = snapshot
        .find_child_by_decl_id(parameters, "parameters/video_audio")
        .unwrap();
    let media = engine.node_id_by_uuid(reference.media.0).unwrap();
    let Some(AppNode::Media(media)) = engine.nodes.get(media) else {
        panic!("missing Video media");
    };
    let control = media.video_control().unwrap();
    let source = control.audio_sources().pop().unwrap();
    assert!(source.binding.voice.routing().unwrap().enabled);

    for (node, enabled, expected) in [
        (folder, false, false),
        (folder, true, true),
        (owner, false, false),
        (folder, false, false),
        (folder, true, false),
        (owner, true, true),
    ] {
        let ack = apply_project_intent(
            &mut engine,
            UiEditIntent::PatchMeta {
                node,
                patch: NodeMetaPatch {
                    enabled: Some(enabled),
                    ..Default::default()
                }
                .into(),
            },
        );
        assert!(ack.success, "{ack:?}");
        assert_eq!(
            source.binding.voice.routing().unwrap().enabled,
            expected,
            "node {node:?} set to {enabled}"
        );
    }
}

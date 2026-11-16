use super::*;
use crate::engine::ProjectNodeMeta;
use crate::node::{GradientNode, NodeMeta, NodeMetaPatch, PresentationHint, SemanticsHint};
use crate::ui_sync::UiEditIntent;

fn constructor_metadata() -> NodeMeta {
    let mut meta = NodeMeta::new("Subject".into());
    meta.tags = vec!["constructor tag".into()];
    meta.semantics = SemanticsHint {
        intent: Some("constructor intent".into()),
        unit: Some("meters".into()),
    };
    meta.presentation.collapsed = true;
    meta
}

fn cleared_metadata() -> NodeMeta {
    let mut meta = constructor_metadata();
    meta.tags.clear();
    meta.semantics = SemanticsHint::default();
    meta.presentation = PresentationHint::default();
    meta
}

fn assert_cleared(record: ProjectNodeMeta) {
    let encoded = serde_json::to_string(&record).unwrap();
    let decoded: ProjectNodeMeta = serde_json::from_str(&encoded).unwrap();
    let mut restored = constructor_metadata();
    decoded.apply_to_runtime(&mut restored, Default::default());
    assert!(restored.tags.is_empty());
    assert_eq!(restored.semantics, SemanticsHint::default());
    assert_eq!(restored.presentation, PresentationHint::default());
}

#[test]
fn full_metadata_preserves_resets_to_default_values() {
    let record = ProjectNodeMeta::from_runtime(&cleared_metadata());
    let encoded = serde_json::to_string(&record).unwrap();
    let decoded: ProjectNodeMeta = serde_json::from_str(&encoded).unwrap();
    let restored = decoded.into_runtime(Default::default());
    assert!(restored.tags.is_empty());
    assert_eq!(restored.semantics, SemanticsHint::default());
    assert_eq!(restored.presentation, PresentationHint::default());
}

#[test]
fn sparse_metadata_preserves_resets_in_declared_and_complete_records() {
    let baseline = ProjectNodeMeta::from_runtime(&constructor_metadata());
    for declared_overlay in [true, false] {
        assert_cleared(crate::app::sparse::project_meta_for_sparse_record(
            &cleared_metadata(),
            Some(&baseline),
            Some(&baseline),
            declared_overlay,
        ));
    }
    assert_cleared(crate::app::sparse::project_meta_delta_from_runtime(
        &cleared_metadata(),
        Some(&baseline),
    ));
}

#[test]
fn expanded_gradient_stop_survives_live_and_immutable_project_capture() {
    let mut engine = Engine::<PreferencesTestNode>::new(Folder::new("Project").into());
    let root = engine.root;
    engine.add_node(GradientNode::new().into(), Some(root));
    engine.apply_edits().unwrap();
    engine.run_tick(std::time::Duration::from_millis(1)).unwrap();
    let gradient = engine.nodes.get(root).unwrap().node_data().first_child.unwrap();
    let stop = engine.process_tree_snapshot().child_ids(gradient)[0];
    let node = engine.nodes.get(stop).unwrap();
    let uuid = node.node_data().meta.uuid;
    assert!(node.node_data().meta.presentation.collapsed);
    let ack = crate::application::apply_project_intent(
        &mut engine,
        UiEditIntent::PatchMeta {
            node: stop,
            patch: NodeMetaPatch {
                presentation: Some(PresentationHint::default()),
                ..Default::default()
            }
            .into(),
        },
    );
    assert!(ack.success, "{ack:?}");

    let graph = ProjectGraphCapture::from_engine(&engine).unwrap();
    for project in [
        capture_sparse_project_file_with_ui_state(&engine, None).unwrap(),
        sparse_project_file_from_capture::<PreferencesTestNode>(&graph, None).unwrap(),
    ] {
        let document = golden_persistence::ApplicationProjectDocument {
            application_id: PreferencesTestNode::application_id().to_owned(),
            project,
        };
        let json =
            golden_persistence::encode_application_project_document(&document, PreferencesTestNode::application_id())
                .unwrap();
        let (loaded, _) = from_sparse_project_json_with_ui_state::<PreferencesTestNode>(&json).unwrap();
        let restored = loaded.node_id_by_uuid(uuid).unwrap();
        assert_eq!(
            loaded.nodes.get(restored).unwrap().node_data().meta.presentation,
            PresentationHint::default()
        );
    }
}

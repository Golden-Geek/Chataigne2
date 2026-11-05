use super::*;
use crate::app::AppNode;
use golden_core::{
    app::{from_sparse_project_json, to_sparse_project_json_pretty, validate_engine_project_candidate},
    node::{Node, NodeMetaPatch},
};

fn metadata(engine: &mut AppEngine, uuid: golden_core::node::NodeUuid, enabled: bool) {
    let node = engine.node_id_by_uuid(uuid).unwrap();
    let ack = apply_project_intent(
        engine,
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
}

#[test]
fn disabled_surface_creation_uses_metadata_without_an_enabled_parameter() {
    let mut engine = fixture();
    let (screen, _) = create(&mut engine);
    let parent = engine.node_id_by_uuid(screen.0).unwrap();
    let surface = super::super::SurfaceNode::from_definition(
        "Disabled".into(),
        SurfaceDefinition {
            enabled: false,
            ..Default::default()
        },
    );
    assert!(!surface.node_data().meta.enabled);
    let uuid = surface.node_data().meta.uuid;
    engine.add_node(surface.into(), Some(parent));
    engine.apply_edits().unwrap();
    let root = engine.node_id_by_uuid(uuid).unwrap();
    let reference = SurfaceReference {
        screen,
        surface: SurfaceId(uuid),
    };
    assert!(!definition(&engine, reference).enabled);
    assert!(!engine.nodes.get(root).unwrap().node_data().effective_enabled);
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    assert!(!saved.contains("render_enabled"));
    let loaded = from_sparse_project_json::<AppNode>(&saved).unwrap();
    validate_engine_project_candidate(&loaded).unwrap();
    assert!(!definition(&loaded, reference).enabled);
}

#[test]
fn surface_configuration_and_generic_metadata_share_history_projection_and_persistence() {
    let mut engine = fixture();
    let (_, reference) = create(&mut engine);
    let original = definition(&engine, reference);
    let mut disabled = original.clone();
    disabled.opacity = 0.25;
    disabled.enabled = false;
    let history = engine.undo_len();
    apply(
        &mut engine,
        ScreenEdit::ConfigureSurface {
            reference,
            definition: disabled.clone(),
        },
    );
    let root = engine.node_id_by_uuid(reference.surface.0).unwrap();
    assert!(!engine.nodes.get(root).unwrap().node_data().meta.enabled);
    assert_eq!(definition(&engine, reference), disabled);
    assert_eq!(engine.undo_len(), history + 1);
    assert!(engine.undo().unwrap());
    assert_eq!(definition(&engine, reference), original);
    assert!(engine.redo().unwrap());
    assert_eq!(definition(&engine, reference), disabled);
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    assert!(!saved.contains("render_enabled"));
    let loaded = from_sparse_project_json::<AppNode>(&saved).unwrap();
    validate_engine_project_candidate(&loaded).unwrap();
    assert_eq!(definition(&loaded, reference), disabled);
    metadata(&mut engine, reference.surface.0, true);
    assert!(definition(&engine, reference).enabled);
    assert!(engine.undo().unwrap());
    assert_eq!(definition(&engine, reference), disabled);
}

#[test]
fn surface_and_ancestor_metadata_retire_render_work_and_source_demand_without_rewriting_authored_state() {
    let mut engine = fixture();
    let (screen, reference) = create(&mut engine);
    let source = super::drop_sources::source(&mut engine);
    apply(
        &mut engine,
        ScreenEdit::ConfigureSurface {
            reference,
            definition: SurfaceDefinition {
                source: Some(source),
                ..Default::default()
            },
        },
    );
    let capture = |engine: &AppEngine| super::super::rendering::capture_screen(engine, screen, 8).unwrap();
    let active = capture(&engine);
    assert!(active.has_content());
    assert_eq!(active.references(), [source]);
    for uuid in [reference.surface.0, screen.0] {
        metadata(&mut engine, uuid, false);
        assert!(!active.is_current());
        assert_eq!(definition(&engine, reference).enabled, uuid != reference.surface.0);
        let disabled = capture(&engine);
        assert!(!disabled.has_content());
        assert!(disabled.references().is_empty());
        assert_eq!(disabled.work_count(golden_video::Extent2D::new(64, 64).unwrap()), 0);
        assert!(
            disabled
                .layers(
                    &Default::default(),
                    golden_video::Extent2D::new(64, 64).unwrap(),
                    &mut super::super::CalibrationAssets::default(),
                )
                .unwrap()
                .is_empty()
        );
        metadata(&mut engine, uuid, true);
        assert!(!disabled.is_current());
        let restored = capture(&engine);
        assert!(restored.has_content());
        assert_eq!(restored.references(), [source]);
    }
}

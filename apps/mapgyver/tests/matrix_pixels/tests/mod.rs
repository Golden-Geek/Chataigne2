use super::*;

#[test]
fn native_scene_reuses_three_by_three_defaults_with_explicit_native_media_and_layer_sizes() {
    let mut engine = golden_core::app::create_new_project_engine::<AppNode>().unwrap();
    engine.apply_edits().unwrap();
    let scene = Scene::new(&mut engine);
    let snapshot = inspect(&mut engine, scene.root.media);
    assert_eq!(snapshot.items.len(), 4);
    assert_eq!(snapshot.columns.len(), 3);
    assert_eq!(snapshot.cells.len(), 6);
    assert_eq!(snapshot.items.last().unwrap().id, scene.outside);
    assert!(
        snapshot
            .items
            .iter()
            .filter_map(|row| row.layer)
            .all(|layer| layer.size == Extent2D::new(32, 24).unwrap() && layer.duration_ms == 4000)
    );
    assert!(
        snapshot
            .cells
            .iter()
            .all(|cell| cell.media.is_some() && cell.definition.source.is_some())
    );
    assert_eq!(
        snapshot
            .cells
            .iter()
            .map(|cell| cell.media.unwrap())
            .collect::<std::collections::HashSet<_>>()
            .len(),
        6,
        "each populated address owns a distinct child media"
    );
    assert_eq!(
        channel(&engine, scene.red).unwrap().definition.size,
        SizePolicy::Fixed {
            extent: Extent2D::new(32, 24).unwrap()
        }
    );
    let saved = golden_core::app::to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded = golden_core::app::from_sparse_project_json::<AppNode>(&saved).unwrap();
    assert_eq!(inspect(&mut loaded, scene.root.media), snapshot);
}

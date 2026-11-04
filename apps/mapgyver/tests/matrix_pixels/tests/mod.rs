use super::*;
mod exposure;

#[test]
fn explicit_native_observation_exports_preserve_default_hidden_policy_and_exact_project_identity() {
    let mut engine = golden_core::app::create_new_project_engine::<AppNode>().unwrap();
    engine.apply_edits().unwrap();
    let scene = Scene::new(&mut engine);
    let before = inspect(&mut engine, scene.root.media);
    assert!(before.items.iter().all(|row| !row.settings.expose_channel));
    expose_outputs(&mut engine, scene.root, &scene.routes());
    let exposed = inspect(&mut engine, scene.root.media);
    for (before, after) in before.items.iter().zip(&exposed.items) {
        assert_eq!(after.id, before.id);
        assert_eq!(after.channel, before.channel);
        assert_eq!(after.layer, before.layer);
        assert_eq!(
            after.settings,
            CompositeSettings {
                expose_channel: scene.routes().contains(&after.channel),
                ..before.settings
            }
        );
        assert_eq!(channel(&engine, after.channel).is_ok(), after.settings.expose_channel);
    }
    let saved = golden_core::app::to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded = golden_core::app::from_sparse_project_json::<AppNode>(&saved).unwrap();
    assert_eq!(inspect(&mut loaded, scene.root.media), exposed);
}

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

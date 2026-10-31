use super::*;
use crate::screens::protocol::PhysicalRectangle;
use golden_video::Extent2D;

fn display(id: &str) -> DisplayInfo {
    DisplayInfo {
        id: id.into(),
        name: "Monitor".into(),
        bounds: PhysicalRectangle {
            left: -1920,
            top: 0,
            extent: Extent2D::new(1920, 1080).unwrap(),
        },
    }
}

#[test]
fn catalog_ignores_order_changes_and_invalidates_on_loss_error_and_recovery() {
    let catalog = Catalog::new(WorkerNotifier::new(|| {}));
    assert!(catalog.request(true).unwrap().displays.is_empty());
    catalog.request(true).unwrap();
    assert!(catalog.take_refresh());
    assert!(!catalog.take_refresh());
    let first = catalog.update(Ok(vec![display("b"), display("a")])).unwrap();
    assert_eq!(first.displays[0].id, "a");
    let guard = catalog.guard();
    let mut engine: crate::app::AppEngine = golden_core::app::create_new_project_engine().unwrap();
    engine.apply_edits().unwrap();
    let mut captured = crate::preview::capture::capture_presentation(&engine, &[], 8, 128, &first.displays).unwrap();
    captured.add_revision_guard(guard.clone());
    assert!(catalog.update(Ok(vec![display("a"), display("b")])).is_none());
    assert!(guard.is_current());
    assert!(captured.is_current());
    let missing = catalog.update(Ok(vec![display("a")])).unwrap();
    assert!(!guard.is_current());
    assert!(
        !captured.is_current(),
        "catalog changes must trigger presentation recapture without a document edit"
    );
    assert!(missing.revision > first.revision);
    let failed = catalog.update(Err("query failed".into())).unwrap();
    assert!(failed.displays.is_empty());
    assert!(failed.error.is_some());
    assert!(catalog.update(Err("query failed".into())).is_none());
    let recovered = catalog.update(Ok(vec![display("b")])).unwrap();
    assert!(recovered.error.is_none());
    assert!(recovered.revision > failed.revision);
}

#[test]
#[ignore = "enumerates actual interactive desktop display paths"]
fn hardware_native_catalog_has_distinct_device_paths_and_physical_bounds() {
    let displays = scan().unwrap();
    assert!(!displays.is_empty());
    assert!(displays.len() <= 32);
    let unique = displays
        .iter()
        .map(|display| &display.id)
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(unique.len(), displays.len());
    for display in &displays {
        assert!(!display.id.is_empty());
        assert!(display.bounds.extent.width() > 0);
        eprintln!(
            "[display-catalog] name={:?} physical={:?}",
            display.name, display.bounds
        );
    }
    if let Some(path) = std::env::var_os("MAPGYVER_DISPLAY_FIXTURE") {
        std::fs::write(path, serde_json::to_vec_pretty(&displays).unwrap()).unwrap();
    }
}

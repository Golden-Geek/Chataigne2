use super::*;

#[test]
fn window_moves_and_display_reassignment_preserve_bindings_but_resize_retires_backing() {
    let mut engine: AppEngine = create_new_project_engine().unwrap();
    engine.apply_edits().unwrap();
    let ScreenEditResult::ScreenCreated { screen, .. } = edit(&mut engine, ScreenEdit::CreateScreen { label: None })
    else {
        panic!()
    };
    configure(&mut engine, screen, "Initial binding", true);
    let mut registry = Targets::default();
    let mut capture = capture_presentation(&engine, &[], 8, 128, &[]).unwrap();
    let spout = registry.assign(&mut capture).unwrap().remove(0);
    let bounds = PhysicalRectangle {
        left: 40,
        top: 80,
        extent: spout.descriptor.extent,
    };
    let mut rectangle = spout.clone();
    rectangle.kind = ResolvedTarget::Rectangle(bounds);
    rectangle.authored = ScreenTarget::Rectangle { bounds };
    rectangle.descriptor.alpha_mode = AlphaMode::Opaque;
    assert!(!spout.same_binding(&rectangle));
    let mut moved = rectangle.clone();
    let next_bounds = PhysicalRectangle {
        left: -1600,
        top: -80,
        ..bounds
    };
    moved.kind = ResolvedTarget::Rectangle(next_bounds);
    moved.authored = ScreenTarget::Rectangle { bounds: next_bounds };
    assert_ne!(rectangle, moved);
    assert!(rectangle.same_binding(&moved));
    assert_ne!(rectangle.window_bounds().unwrap(), moved.window_bounds().unwrap());
    let mut reassigned = moved.clone();
    reassigned.kind = ResolvedTarget::Fullscreen {
        display: "second display".into(),
        bounds: next_bounds,
    };
    reassigned.authored = ScreenTarget::Fullscreen {
        display: "second display".into(),
    };
    assert!(moved.same_binding(&reassigned));
    let mut different_rate = reassigned.clone();
    different_rate.frame_rate = golden_time::RationalRate::new(30, 1).unwrap();
    assert!(
        !reassigned.same_binding(&different_rate),
        "a changed immutable cadence rebinds after normal retirement"
    );
    let mut resized = reassigned.clone();
    resized.descriptor.extent = Extent2D::new(640, 360).unwrap();
    assert!(
        !reassigned.same_binding(&resized),
        "new backing extent must be admitted after retirement"
    );
    let mut another_project = reassigned.clone();
    another_project.project = uuid::Uuid::new_v4();
    assert!(!reassigned.same_binding(&another_project));
    assert!(!same_bindings(&[reassigned.clone()], &[]));
    assert!(same_bindings(&[rectangle], &[reassigned]));
    let mut renamed = spout.clone();
    renamed.kind = ResolvedTarget::Spout {
        sender: "Changed sender".into(),
        extent: spout.descriptor.extent,
    };
    assert!(!spout.same_binding(&renamed));
}

use super::*;
use crate::app::AppNode;
use golden_video::Extent2D;
use std::sync::Arc;

#[test]
fn pixel_samples_use_output_extent_and_coalesce_without_losing_distance_or_history() {
    let mut engine = fixture();
    let (screen, reference) = create(&mut engine);
    for target in [
        ScreenTarget::Spout {
            sender: "Pixel test".into(),
            extent: Extent2D::new(640, 360).unwrap(),
        },
        ScreenTarget::Rectangle {
            bounds: PhysicalRectangle {
                left: -100,
                top: 50,
                extent: Extent2D::new(640, 360).unwrap(),
            },
        },
    ] {
        apply(
            &mut engine,
            ScreenEdit::ConfigureScreen {
                screen,
                definition: ScreenDefinition {
                    target: Some(target),
                    enabled: false,
                    anti_aliasing: Default::default(),
                },
            },
        );
        let original = definition(&engine, reference);
        let history = engine.undo_len();
        super::gestures::session(&mut engine, true);
        for x in [1, 10, 100] {
            apply(
                &mut engine,
                ScreenEdit::EditSurfaceGeometry {
                    reference,
                    edit: SurfaceGeometryEdit::QuadCornerPixels {
                        corner: 0,
                        origin: original.quad.corners[0],
                        pixels: PixelOffset { x, y: -10 },
                    },
                },
            );
            let changed = definition(&engine, reference);
            assert!((changed.quad.corners[0].x - original.quad.corners[0].x - x as f32 / 640.0).abs() < 1e-6);
            assert!((changed.quad.corners[0].y - original.quad.corners[0].y + 10.0 / 360.0).abs() < 1e-6);
            assert_eq!(&changed.quad.corners[1..], &original.quad.corners[1..]);
            assert_eq!(changed.crop, original.crop);
        }
        super::gestures::session(&mut engine, false);
        let final_state = definition(&engine, reference);
        assert_eq!(engine.undo_len(), history + 1);
        assert!(engine.undo().unwrap());
        assert_eq!(definition(&engine, reference), original);
        assert!(engine.redo().unwrap());
        assert_eq!(definition(&engine, reference), final_state);
    }
}

#[test]
fn fullscreen_pixel_extent_uses_cached_intended_identity_and_rejects_unavailable_or_bad_inputs() {
    let mut engine = fixture();
    let (screen, reference) = create(&mut engine);
    let display = DisplayInfo {
        id: "pixels-display".into(),
        name: "Pixels".into(),
        edid: None,
        index: 1,
        primary: true,
        bounds: PhysicalRectangle {
            left: -800,
            top: 0,
            extent: Extent2D::new(800, 600).unwrap(),
        },
    };
    let manager = super::super::authoring::manager(&engine).unwrap();
    let Some(AppNode::ScreenManager(node)) = engine.nodes.get_mut(manager) else {
        panic!()
    };
    node.set_displays(Arc::from([display.clone()]));
    apply(
        &mut engine,
        ScreenEdit::ConfigureScreen {
            screen,
            definition: ScreenDefinition {
                target: Some(ScreenTarget::Fullscreen { display: display.id }),
                enabled: false,
                anti_aliasing: Default::default(),
            },
        },
    );
    let original = definition(&engine, reference);
    let nudge = |corner| ScreenEdit::EditSurfaceGeometry {
        reference,
        edit: SurfaceGeometryEdit::QuadCornerPixels {
            corner,
            origin: original.quad.corners[0],
            pixels: PixelOffset { x: 1, y: 10 },
        },
    };
    apply(&mut engine, nudge(0));
    let changed = definition(&engine, reference);
    assert!((changed.quad.corners[0].x - original.quad.corners[0].x - 1.0 / 800.0).abs() < 1e-6);
    assert!((changed.quad.corners[0].y - original.quad.corners[0].y - 10.0 / 600.0).abs() < 1e-6);
    let history = engine.undo_len();
    assert!(!invoke(&mut engine, nudge(4)).success);
    assert_eq!(definition(&engine, reference), changed);
    assert_eq!(engine.undo_len(), history);
    let Some(AppNode::ScreenManager(node)) = engine.nodes.get_mut(manager) else {
        panic!()
    };
    node.set_displays(Arc::from([]));
    assert!(!invoke(&mut engine, nudge(0)).success);
    assert_eq!(definition(&engine, reference), changed);
    apply(
        &mut engine,
        ScreenEdit::ConfigureScreen {
            screen,
            definition: ScreenDefinition {
                target: None,
                enabled: false,
                anti_aliasing: Default::default(),
            },
        },
    );
    let history = engine.undo_len();
    assert!(!invoke(&mut engine, nudge(0)).success);
    assert_eq!(engine.undo_len(), history);
}

use super::*;
use crate::{
    app::AppNode,
    media::protocol::{MediaEdit, SizePolicy},
};
use golden_core::app::{from_sparse_project_json, to_sparse_project_json_pretty};
use golden_video::Extent2D;

fn size(engine: &mut AppEngine, source: crate::media::protocol::ChannelReference, width: u32, height: u32) {
    super::drop_sources::media(
        engine,
        MediaEdit::ConfigureMediaResolution {
            media: source.media,
            size: SizePolicy::Fixed {
                extent: Extent2D::new(width, height).unwrap(),
            },
        },
    );
}
#[test]
fn fill_and_fit_use_backend_target_and_cropped_source_aspect_with_history_and_roundtrip() {
    let mut engine = fixture();
    let (screen, reference) = create(&mut engine);
    let source = super::drop_sources::source(&mut engine);
    apply(
        &mut engine,
        ScreenEdit::ConfigureScreen {
            screen,
            definition: ScreenDefinition {
                target: Some(ScreenTarget::Spout {
                    sender: "Fit".into(),
                    extent: Extent2D::new(800, 600).unwrap(),
                }),
                enabled: false,
            },
        },
    );
    let mut original = definition(&engine, reference);
    original.source = Some(source);
    original.opacity = 0.4;
    original.enabled = false;
    original.calibration = Calibration::Grid;
    original.quad.corners.swap(1, 2);
    for (width, height, crop, expected) in [
        (1600, 900, UnitRect::FULL, [0.0, 0.125, 1.0, 0.875]),
        (600, 1200, UnitRect::FULL, [0.3125, 0.0, 0.6875, 1.0]),
        (
            1600,
            900,
            UnitRect {
                x: 0.25,
                y: 0.0,
                width: 0.5,
                height: 1.0,
            },
            [1.0 / 6.0, 0.0, 5.0 / 6.0, 1.0],
        ),
    ] {
        size(&mut engine, source, width, height);
        original.crop = crop;
        apply(
            &mut engine,
            ScreenEdit::ConfigureSurface {
                reference,
                definition: original.clone(),
            },
        );
        let history = engine.undo_len();
        let ScreenEditResult::SurfaceChanged { snapshot } = apply(
            &mut engine,
            ScreenEdit::FitSurface {
                reference,
                mode: SurfaceFitMode::Fit,
            },
        ) else {
            panic!()
        };
        let corners = snapshot.definition.quad.corners;
        for (actual, expected) in [corners[0].x, corners[0].y, corners[2].x, corners[2].y]
            .into_iter()
            .zip(expected)
        {
            assert!((actual - expected).abs() < 1e-6);
        }
        let mut unchanged = snapshot.definition.clone();
        unchanged.quad = original.quad;
        assert_eq!(unchanged, original);
        assert_eq!(engine.undo_len(), history + 1);
        assert!(engine.undo().unwrap());
        assert_eq!(definition(&engine, reference), original);
        assert!(engine.redo().unwrap());
        assert_eq!(definition(&engine, reference), snapshot.definition);
        let saved = to_sparse_project_json_pretty(&engine).unwrap();
        let mut loaded = from_sparse_project_json::<AppNode>(&saved).unwrap();
        loaded.apply_edits().unwrap();
        assert_eq!(definition(&loaded, reference), snapshot.definition);
        let before_fill = engine.undo_len();
        apply(
            &mut engine,
            ScreenEdit::FitSurface {
                reference,
                mode: SurfaceFitMode::Fill,
            },
        );
        let mut filled = original.clone();
        filled.quad = Quad::default();
        assert_eq!(definition(&engine, reference), filled);
        assert_eq!(engine.undo_len(), before_fill + 1);
        assert!(engine.undo().unwrap());
        assert_eq!(definition(&engine, reference), snapshot.definition);
    }
}

#[test]
fn fit_preserves_reference_resolution_overrides_and_rejects_unknown_inputs_atomically() {
    let mut engine = fixture();
    let (screen, reference) = create(&mut engine);
    apply(
        &mut engine,
        ScreenEdit::ConfigureScreen {
            screen,
            definition: ScreenDefinition {
                target: Some(ScreenTarget::Rectangle {
                    bounds: PhysicalRectangle {
                        left: -800,
                        top: 40,
                        extent: Extent2D::new(800, 600).unwrap(),
                    },
                }),
                enabled: false,
            },
        },
    );
    let source = super::drop_sources::source(&mut engine);
    size(&mut engine, source, 1600, 900);
    let crate::media::protocol::MediaEditResult::ReferenceCreated { media } = super::drop_sources::media(
        &mut engine,
        MediaEdit::CreateReference {
            target: Some(source.media),
            label: None,
        },
    ) else {
        panic!()
    };
    let alias = crate::media::protocol::ChannelReference { media, ..source };
    let mut original = definition(&engine, reference);
    original.source = Some(alias);
    apply(
        &mut engine,
        ScreenEdit::ConfigureSurface {
            reference,
            definition: original.clone(),
        },
    );
    apply(
        &mut engine,
        ScreenEdit::FitSurface {
            reference,
            mode: SurfaceFitMode::Fit,
        },
    );
    assert!((definition(&engine, reference).quad.corners[0].y - 0.125).abs() < 1e-6);
    size(&mut engine, alias, 600, 1200);
    apply(
        &mut engine,
        ScreenEdit::FitSurface {
            reference,
            mode: SurfaceFitMode::Fit,
        },
    );
    assert!((definition(&engine, reference).quad.corners[0].x - 0.3125).abs() < 1e-6);
    let crate::media::protocol::MediaEditResult::Created { reference: unknown } = super::drop_sources::media(
        &mut engine,
        MediaEdit::CreateMedia {
            media_kind: crate::media::protocol::MediaKind::SharedTexture,
            label: None,
        },
    ) else {
        panic!()
    };
    original.source = Some(unknown);
    apply(
        &mut engine,
        ScreenEdit::ConfigureSurface {
            reference,
            definition: original.clone(),
        },
    );
    for source in [Some(unknown), None] {
        original.source = source;
        apply(
            &mut engine,
            ScreenEdit::ConfigureSurface {
                reference,
                definition: original.clone(),
            },
        );
        let saved = to_sparse_project_json_pretty(&engine).unwrap();
        let history = engine.undo_len();
        assert!(
            !invoke(
                &mut engine,
                ScreenEdit::FitSurface {
                    reference,
                    mode: SurfaceFitMode::Fit
                }
            )
            .success
        );
        assert_eq!(engine.undo_len(), history);
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), saved);
        assert!(
            invoke(
                &mut engine,
                ScreenEdit::FitSurface {
                    reference,
                    mode: SurfaceFitMode::Fill
                }
            )
            .success
        );
    }
    apply(
        &mut engine,
        ScreenEdit::ConfigureScreen {
            screen,
            definition: ScreenDefinition::default(),
        },
    );
    let history = engine.undo_len();
    assert!(
        !invoke(
            &mut engine,
            ScreenEdit::FitSurface {
                reference,
                mode: SurfaceFitMode::Fit
            }
        )
        .success
    );
    assert_eq!(engine.undo_len(), history);
}

#[test]
fn fit_uses_cached_fullscreen_extent_and_rejects_lost_display_without_history_change() {
    let mut engine = fixture();
    let (screen, reference) = create(&mut engine);
    let source = super::drop_sources::source(&mut engine);
    let display = DisplayInfo {
        id: "fit-display".into(),
        name: "Fit display".into(),
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
    node.set_displays(std::sync::Arc::from([display.clone()]));
    apply(
        &mut engine,
        ScreenEdit::ConfigureScreen {
            screen,
            definition: ScreenDefinition {
                target: Some(ScreenTarget::Fullscreen { display: display.id }),
                enabled: false,
            },
        },
    );
    let mut original = definition(&engine, reference);
    original.source = Some(source);
    apply(
        &mut engine,
        ScreenEdit::ConfigureSurface {
            reference,
            definition: original,
        },
    );
    apply(
        &mut engine,
        ScreenEdit::FitSurface {
            reference,
            mode: SurfaceFitMode::Fit,
        },
    );
    assert!((definition(&engine, reference).quad.corners[0].y - 0.125).abs() < 1e-6);
    let Some(AppNode::ScreenManager(node)) = engine.nodes.get_mut(manager) else {
        panic!()
    };
    node.set_displays(std::sync::Arc::from([]));
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    assert!(
        !invoke(
            &mut engine,
            ScreenEdit::FitSurface {
                reference,
                mode: SurfaceFitMode::Fit
            }
        )
        .success
    );
    assert_eq!(engine.undo_len(), history);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), saved);
}

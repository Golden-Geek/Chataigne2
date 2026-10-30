use super::*;
use crate::{
    app::AppNode,
    media::{
        apply_operation,
        protocol::{ChannelReference, MEDIA_OPERATION, MediaEdit, MediaEditResult, MediaKind},
    },
};
use golden_core::{
    app::{from_sparse_project_json, to_sparse_project_json_pretty},
    node::Node,
};
use golden_video::Extent2D;

#[test]
fn creation_batches_default_surface_and_allocates_stable_backend_labels() {
    let mut engine = fixture();
    let before = engine.undo_len();
    let (first, surface) = create(&mut engine);
    assert_eq!(engine.undo_len(), before + 1);
    let snapshot = super::super::inspect(&engine).unwrap();
    assert_eq!(snapshot.screens[0].label, "Screen");
    assert_eq!(snapshot.screens[0].definition, ScreenDefinition::default());
    assert_eq!(snapshot.screens[0].surfaces[0].definition, SurfaceDefinition::default());
    assert_eq!(surface.screen, first);
    assert!(engine.undo().unwrap());
    assert!(super::super::inspect(&engine).unwrap().screens.is_empty());
    assert!(engine.redo().unwrap());
    assert_eq!(super::super::inspect(&engine).unwrap(), snapshot);
    let (second, _) = create(&mut engine);
    assert_ne!(first, second);
    assert_eq!(super::super::inspect(&engine).unwrap().screens[1].label, "Screen 2");
}

#[test]
fn targets_and_ordered_surfaces_roundtrip_with_atomic_history() {
    let mut engine = fixture();
    let (screen, first) = create(&mut engine);
    let ScreenEditResult::SurfaceCreated { reference: second } =
        apply(&mut engine, ScreenEdit::AddSurface { screen, label: None })
    else {
        panic!()
    };
    let (other, _) = create(&mut engine);
    apply(
        &mut engine,
        ScreenEdit::RenameScreen {
            screen,
            label: "Mapped wall".into(),
        },
    );
    apply(
        &mut engine,
        ScreenEdit::RenameSurface {
            reference: first,
            label: "Left".into(),
        },
    );
    apply(
        &mut engine,
        ScreenEdit::ReorderSurface {
            reference: first,
            after: Some(second.surface),
        },
    );
    apply(
        &mut engine,
        ScreenEdit::ReorderScreen {
            screen: other,
            after: None,
        },
    );
    for target in [
        ScreenTarget::Fullscreen {
            display: "disconnected-stable-display-path".into(),
        },
        ScreenTarget::Spout {
            sender: "MapGyver output".into(),
            extent: Extent2D::new(1920, 1080).unwrap(),
        },
        ScreenTarget::Rectangle {
            bounds: PhysicalRectangle {
                left: -1920,
                top: -200,
                extent: Extent2D::new(1920, 1080).unwrap(),
            },
        },
    ] {
        let before = engine.undo_len();
        apply(
            &mut engine,
            ScreenEdit::ConfigureScreen {
                screen,
                definition: ScreenDefinition {
                    target: Some(target),
                    enabled: true,
                },
            },
        );
        assert_eq!(engine.undo_len(), before + 1);
        let saved = super::super::inspect(&engine).unwrap();
        assert_eq!(saved.screens[0].id, other);
        assert_eq!(saved.screens[1].surfaces[1].reference, first);
        let json = to_sparse_project_json_pretty(&engine).unwrap();
        for forbidden in ["hwnd", "shared_handle", "device_generation"] {
            assert!(!json.contains(forbidden));
        }
        let mut loaded = from_sparse_project_json::<AppNode>(&json).unwrap();
        loaded.apply_edits().unwrap();
        super::super::validate_project(&loaded).unwrap();
        assert_eq!(super::super::inspect(&loaded).unwrap(), saved);
    }
    let before = super::super::inspect(&engine).unwrap();
    apply(&mut engine, ScreenEdit::RemoveSurface { reference: first });
    assert_eq!(super::super::inspect(&engine).unwrap().screens[1].surfaces.len(), 1);
    assert!(engine.undo().unwrap());
    assert_eq!(super::super::inspect(&engine).unwrap(), before);
    apply(&mut engine, ScreenEdit::RemoveScreen { screen });
    assert!(engine.node_id_by_uuid(first.surface.0).is_none());
    assert!(engine.undo().unwrap());
    assert_eq!(super::super::inspect(&engine).unwrap(), before);
    apply(&mut engine, ScreenEdit::RemoveSurface { reference: first });
    apply(&mut engine, ScreenEdit::RemoveSurface { reference: second });
    super::super::validate_project(&engine).unwrap();
    assert!(super::super::inspect(&engine).unwrap().screens[1].surfaces.is_empty());
}

#[test]
fn invalid_edits_leave_project_and_history_untouched() {
    let mut engine = fixture();
    let (screen, first) = create(&mut engine);
    let (_, other) = create(&mut engine);
    let mut crop = SurfaceDefinition::default();
    crop.crop.width = 2.0;
    let mut quad = SurfaceDefinition::default();
    quad.quad.corners.swap(0, 1);
    let alpha = SurfaceDefinition {
        opacity: -0.1,
        ..Default::default()
    };
    let missing = SurfaceDefinition {
        source: Some(ChannelReference {
            media: crate::media::protocol::MediaId(golden_core::model::NodeUuid(uuid::Uuid::new_v4())),
            channel: crate::media::protocol::ChannelId(golden_core::model::NodeUuid(uuid::Uuid::new_v4())),
        }),
        ..Default::default()
    };
    let invalid = vec![
        ScreenEdit::ConfigureSurface {
            reference: first,
            definition: crop,
        },
        ScreenEdit::ConfigureSurface {
            reference: first,
            definition: quad,
        },
        ScreenEdit::ConfigureSurface {
            reference: first,
            definition: alpha,
        },
        ScreenEdit::ConfigureSurface {
            reference: first,
            definition: missing,
        },
        ScreenEdit::ReorderSurface {
            reference: first,
            after: Some(other.surface),
        },
        ScreenEdit::ReorderSurface {
            reference: first,
            after: Some(first.surface),
        },
        ScreenEdit::RemoveSurface {
            reference: SurfaceReference {
                surface: other.surface,
                ..first
            },
        },
        ScreenEdit::RenameScreen {
            screen,
            label: " ".into(),
        },
        ScreenEdit::ConfigureScreen {
            screen,
            definition: ScreenDefinition {
                target: Some(ScreenTarget::Spout {
                    sender: "".into(),
                    extent: Extent2D::new(1, 1).unwrap(),
                }),
                enabled: true,
            },
        },
        ScreenEdit::ConfigureScreen {
            screen,
            definition: ScreenDefinition {
                target: Some(ScreenTarget::Rectangle {
                    bounds: PhysicalRectangle {
                        left: i32::MAX,
                        top: 0,
                        extent: Extent2D::new(2, 2).unwrap(),
                    },
                }),
                enabled: true,
            },
        },
    ];
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    for edit in invalid {
        assert!(!invoke(&mut engine, edit.clone()).success, "{edit:?}");
        assert_eq!(engine.undo_len(), history);
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), saved);
    }
}

#[test]
fn calibration_and_source_deletion_preserve_routing_and_allow_missing_source_edits() {
    let mut engine = fixture();
    let (_, reference) = create(&mut engine);
    let result = apply_operation(
        &mut engine,
        MEDIA_OPERATION,
        serde_json::to_value(MediaEdit::CreateMedia {
            media_kind: MediaKind::SolidColor,
            label: None,
        })
        .unwrap(),
    )
    .unwrap();
    engine.apply_edits().unwrap();
    let MediaEditResult::Created { reference: source } = serde_json::from_value(result).unwrap() else {
        panic!()
    };
    let mut authored = definition(&engine, reference);
    authored.source = Some(source);
    authored.crop = Crop {
        x: 0.25,
        y: 0.0,
        width: 0.5,
        height: 1.0,
    };
    authored.opacity = 0.4;
    authored.calibration = Calibration::Grid;
    apply(
        &mut engine,
        ScreenEdit::ConfigureSurface {
            reference,
            definition: authored.clone(),
        },
    );
    let remove = apply_project_intent(
        &mut engine,
        UiEditIntent::InvokeAppOperation {
            namespace: crate::media::protocol::MEDIA_NAMESPACE.into(),
            operation: MEDIA_OPERATION.into(),
            payload: serde_json::to_value(MediaEdit::RemoveMedia { media: source.media }).unwrap(),
        },
    );
    assert!(remove.success);
    assert!(crate::media::channel(&engine, source).is_err());
    assert_eq!(definition(&engine, reference), authored);
    let mut restored = authored.clone();
    restored.calibration = Calibration::Off;
    apply(
        &mut engine,
        ScreenEdit::ConfigureSurface {
            reference,
            definition: restored.clone(),
        },
    );
    assert_eq!(definition(&engine, reference).source, Some(source));
    let json = to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded = from_sparse_project_json::<AppNode>(&json).unwrap();
    loaded.apply_edits().unwrap();
    super::super::validate_project(&loaded).unwrap();
    assert_eq!(definition(&loaded, reference), restored);
    assert!(engine.undo().unwrap()); // Calibration edit.
    assert!(engine.undo().unwrap()); // Source deletion.
    assert!(crate::media::channel(&engine, source).is_ok());
    assert_eq!(definition(&engine, reference), authored);
}

#[test]
fn project_validation_rejects_surface_reparented_outside_screen() {
    let mut engine = fixture();
    let (_, reference) = create(&mut engine);
    let node = engine.node_id_by_uuid(reference.surface.0).unwrap();
    engine.edits.push(golden_core::edit::Edit::MoveNode {
        node,
        new_parent: engine.root,
        new_prev_sibling: None,
    });
    engine.apply_edits().unwrap();
    assert!(super::super::validate_project(&engine).is_err());
    assert!(matches!(engine.nodes.get(node), Some(AppNode::Surface(_))));
    assert_eq!(engine.nodes.get(node).unwrap().node_data().parent, Some(engine.root));
}

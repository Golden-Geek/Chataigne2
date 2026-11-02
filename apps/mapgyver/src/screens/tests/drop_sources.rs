use super::*;
use crate::{
    app::AppNode,
    media::protocol::{ChannelReference, MEDIA_NAMESPACE, MEDIA_OPERATION, MediaEdit, MediaEditResult, MediaKind},
};
use golden_core::{
    app::{from_sparse_project_json, to_sparse_project_json_pretty},
    model::NodeUuid,
};

pub(super) fn media(engine: &mut AppEngine, edit: MediaEdit) -> MediaEditResult {
    let ack = apply_project_intent(
        engine,
        UiEditIntent::InvokeAppOperation {
            namespace: MEDIA_NAMESPACE.into(),
            operation: MEDIA_OPERATION.into(),
            payload: serde_json::to_value(edit).unwrap(),
        },
    );
    assert!(ack.success, "{ack:?}");
    serde_json::from_value(ack.result.unwrap().value).unwrap()
}

pub(super) fn source(engine: &mut AppEngine) -> ChannelReference {
    let MediaEditResult::Created { reference } = media(
        engine,
        MediaEdit::CreateMedia {
            media_kind: MediaKind::SolidColor,
            label: None,
        },
    ) else {
        panic!()
    };
    reference
}

fn two_channels(engine: &mut AppEngine) -> (ChannelReference, ChannelReference) {
    let first = source(engine);
    let MediaEditResult::Created { reference: second } = media(
        engine,
        MediaEdit::AddChannel {
            media: first.media,
            label: None,
        },
    ) else {
        panic!()
    };
    (first, second)
}

#[test]
fn dropping_media_creates_one_default_surface_with_its_first_ordered_channel() {
    let mut engine = fixture();
    let (screen, _) = create(&mut engine);
    let (first, second) = two_channels(&mut engine);
    media(
        &mut engine,
        MediaEdit::ReorderChannel {
            reference: second,
            after: None,
        },
    );
    let original = super::super::inspect(&engine).unwrap();
    let history = engine.undo_len();
    let ScreenEditResult::SurfaceCreated { reference } = apply(
        &mut engine,
        ScreenEdit::DropSource {
            screen,
            surface: None,
            source: first.media.0,
        },
    ) else {
        panic!()
    };
    assert_eq!(reference.screen, screen);
    assert_eq!(engine.undo_len(), history + 1);
    assert_eq!(
        definition(&engine, reference),
        SurfaceDefinition {
            source: Some(second),
            ..Default::default()
        }
    );
    let dropped = super::super::inspect(&engine).unwrap();
    assert_eq!(
        dropped.screens[0].surfaces.len(),
        original.screens[0].surfaces.len() + 1
    );
    assert_eq!(dropped.screens[0].surfaces[0], original.screens[0].surfaces[0]);
    assert!(engine.undo().unwrap());
    assert!(engine.node_id_by_uuid(reference.surface.0).is_none());
    assert_eq!(super::super::inspect(&engine).unwrap(), original);
    assert!(engine.redo().unwrap());
    assert_eq!(super::super::inspect(&engine).unwrap(), dropped);
    let json = to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded = from_sparse_project_json::<AppNode>(&json).unwrap();
    loaded.apply_edits().unwrap();
    super::super::validate_project(&loaded).unwrap();
    assert_eq!(super::super::inspect(&loaded).unwrap(), dropped);
}

#[test]
fn dropping_a_specific_channel_replaces_only_source_and_undo_restores_every_setting() {
    let mut engine = fixture();
    let (screen, reference) = create(&mut engine);
    let (first, second) = two_channels(&mut engine);
    let mut authored = SurfaceDefinition {
        source: Some(first),
        crop: UnitRect {
            x: 0.1,
            y: 0.2,
            width: 0.6,
            height: 0.7,
        },
        opacity: 0.37,
        calibration: Calibration::ColorBars,
        enabled: false,
        ..Default::default()
    };
    authored.quad.corners[0] = Point { x: -0.2, y: 0.1 };
    apply(
        &mut engine,
        ScreenEdit::ConfigureSurface {
            reference,
            definition: authored.clone(),
        },
    );
    let original = super::super::inspect(&engine).unwrap();
    let nodes = engine.nodes.len();
    let history = engine.undo_len();
    let ScreenEditResult::SurfaceChanged { snapshot } = apply(
        &mut engine,
        ScreenEdit::DropSource {
            screen,
            surface: Some(reference.surface),
            source: second.channel.0,
        },
    ) else {
        panic!()
    };
    let expected = SurfaceDefinition {
        source: Some(second),
        ..authored
    };
    assert_eq!(snapshot.reference, reference);
    assert_eq!(snapshot.definition, expected);
    assert_eq!(definition(&engine, reference), expected);
    assert_eq!(engine.nodes.len(), nodes);
    assert_eq!(engine.undo_len(), history + 1);
    let dropped = super::super::inspect(&engine).unwrap();
    assert!(engine.undo().unwrap());
    assert_eq!(super::super::inspect(&engine).unwrap(), original);
    assert!(engine.redo().unwrap());
    assert_eq!(super::super::inspect(&engine).unwrap(), dropped);
    apply(
        &mut engine,
        ScreenEdit::DropSource {
            screen,
            surface: Some(reference.surface),
            source: second.channel.0,
        },
    );
    assert_eq!(
        engine.undo_len(),
        history + 1,
        "dropping the same source has no history"
    );
    assert_eq!(super::super::inspect(&engine).unwrap(), dropped);
}

#[test]
fn invalid_drop_sources_and_destinations_leave_project_and_history_untouched() {
    let mut engine = fixture();
    let (screen, reference) = create(&mut engine);
    let (_, other) = create(&mut engine);
    let valid = source(&mut engine);
    let removed = source(&mut engine);
    media(&mut engine, MediaEdit::RemoveMedia { media: removed.media });
    let missing = NodeUuid(uuid::Uuid::new_v4());
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    for edit in [
        ScreenEdit::DropSource {
            screen,
            surface: None,
            source: missing,
        },
        ScreenEdit::DropSource {
            screen,
            surface: Some(reference.surface),
            source: removed.channel.0,
        },
        ScreenEdit::DropSource {
            screen,
            surface: None,
            source: screen.0,
        },
        ScreenEdit::DropSource {
            screen: ScreenId(missing),
            surface: None,
            source: valid.media.0,
        },
        ScreenEdit::DropSource {
            screen,
            surface: Some(SurfaceId(missing)),
            source: valid.channel.0,
        },
        ScreenEdit::DropSource {
            screen,
            surface: Some(other.surface),
            source: valid.channel.0,
        },
    ] {
        let ack = invoke(&mut engine, edit);
        assert!(!ack.success, "{ack:?}");
        assert_eq!(engine.undo_len(), history);
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), saved);
    }
    let malformed = apply_project_intent(
        &mut engine,
        UiEditIntent::InvokeAppOperation {
            namespace: SCREEN_NAMESPACE.into(),
            operation: SCREEN_OPERATION.into(),
            payload: serde_json::json!({"kind": "DropSource", "screen": screen, "surface": null, "source": "malformed"}),
        },
    );
    assert!(!malformed.success);
    assert_eq!(engine.undo_len(), history);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), saved);
}

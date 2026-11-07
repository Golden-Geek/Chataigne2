use super::*;
use crate::{app::AppNode, media::channel};
use golden_core::{
    app::{from_sparse_project_json, to_sparse_project_json_pretty},
    node::{Node, NodeId},
    parameter::{ParamValue, ParameterEventBehaviour},
};

fn field(engine: &AppEngine, owner: MediaId) -> Option<NodeId> {
    let owner = engine.node_id_by_uuid(owner.0).unwrap();
    engine.nodes.iter().find_map(|(id, node)| {
        let parent = node.node_data().parent.and_then(|id| engine.nodes.get(id));
        (node.node_data().meta.decl_id.0 == "parameters/fit_mode"
            && parent.is_some_and(|folder| folder.node_data().parent == Some(owner)))
        .then_some(id)
    })
}

#[test]
fn fit_mode_is_media_owned_typed_shared_undoable_and_persisted() {
    for kind in [MediaKind::Picture, MediaKind::SharedTexture] {
        let mut engine = fixture();
        let first = create(&mut engine, kind);
        let MediaEditResult::Created { reference: peer } = apply(
            &mut engine,
            MediaEdit::AddChannel {
                media: first.media,
                label: None,
            },
        ) else {
            panic!()
        };
        let parameter = field(&engine, first.media).unwrap();
        let uuid = engine.nodes.get(parameter).unwrap().node_data().meta.uuid;
        for mode in [MediaFitMode::Fill, MediaFitMode::Stretch, MediaFitMode::Fit] {
            let before = channel(&engine, first).unwrap().definition;
            let ack = apply_project_intent(
                &mut engine,
                UiEditIntent::SetParam {
                    node: parameter,
                    value: ParamValue::Enum(mode.name().into()),
                    behaviour: ParameterEventBehaviour::Append,
                },
            );
            assert!(ack.success, "{ack:?}");
            for reference in [first, peer] {
                let definition = channel(&engine, reference).unwrap().definition;
                assert_eq!(definition.fit_mode, mode);
                assert_eq!(definition.size, SizePolicy::Automatic);
            }
            assert!(engine.undo().unwrap());
            assert_eq!(channel(&engine, first).unwrap().definition, before);
            assert!(engine.redo().unwrap());
        }
        apply(
            &mut engine,
            MediaEdit::ConfigureMediaFitMode {
                media: first.media,
                mode: MediaFitMode::Fill,
            },
        );
        let saved = to_sparse_project_json_pretty(&engine).unwrap();
        let mut loaded = from_sparse_project_json::<AppNode>(&saved).unwrap();
        loaded.apply_edits().unwrap();
        assert!(loaded.node_id_by_uuid(uuid).is_some());
        for reference in [first, peer] {
            assert_eq!(
                channel(&loaded, reference).unwrap().definition.fit_mode,
                MediaFitMode::Fill
            );
        }
        let invalid = apply_project_intent(
            &mut engine,
            UiEditIntent::SetParam {
                node: parameter,
                value: ParamValue::Enum("Unknown".into()),
                behaviour: ParameterEventBehaviour::Append,
            },
        );
        assert!(!invalid.success);
    }
}

#[test]
fn fit_mode_is_absent_on_generated_canvases_and_solid_color_and_rejected_by_intents() {
    for kind in [MediaKind::SolidColor, MediaKind::Composition, MediaKind::Matrix] {
        let mut engine = fixture();
        let reference = create(&mut engine, kind);
        assert!(field(&engine, reference.media).is_none());
        let history = engine.undo_len();
        assert!(
            !invoke(
                &mut engine,
                MediaEdit::ConfigureMediaFitMode {
                    media: reference.media,
                    mode: MediaFitMode::Fill,
                }
            )
            .success
        );
        assert_eq!(engine.undo_len(), history);
    }
}

#[test]
fn reference_fit_mode_persists_without_changing_virtual_channel_identity_or_auto_resolution() {
    let mut engine = fixture();
    let target = create(&mut engine, MediaKind::Picture);
    let MediaEditResult::ReferenceCreated { media } = apply(
        &mut engine,
        MediaEdit::CreateReference {
            target: Some(target.media),
            label: None,
        },
    ) else {
        panic!()
    };
    apply(
        &mut engine,
        MediaEdit::ConfigureMediaFitMode {
            media,
            mode: MediaFitMode::Stretch,
        },
    );
    let reference = crate::media::resolve_source_node(&engine, media.0).unwrap();
    assert_eq!(
        channel(&engine, reference).unwrap().definition.fit_mode,
        MediaFitMode::Stretch
    );
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded = from_sparse_project_json::<AppNode>(&saved).unwrap();
    loaded.apply_edits().unwrap();
    assert_eq!(
        channel(&loaded, reference).unwrap().definition,
        channel(&engine, reference).unwrap().definition
    );
}

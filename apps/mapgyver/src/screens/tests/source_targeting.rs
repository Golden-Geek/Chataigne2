use super::drop_sources::{media, source};
use super::parameters::{parameters, set};
use super::*;
use crate::app::AppNode;
use crate::media::protocol::{MediaEdit, MediaEditResult, MediaKind};
use golden_core::{
    app::{from_sparse_project_json, to_sparse_project_json_pretty},
    edit::Edit,
    node::{Node, NodeId, NodeReference, NodeUuid},
    parameter::{ParamValue, ParameterEventBehaviour},
    process_ctx::ExecutionPhase,
};

fn fields(engine: &AppEngine, surface: SurfaceReference) -> (NodeId, NodeId) {
    let fields = parameters(engine, engine.node_id_by_uuid(surface.surface.0).unwrap());
    (fields["parameters/source_media"], fields["parameters/source_channel"])
}
fn value(uuid: NodeUuid) -> ParamValue {
    ParamValue::Reference(NodeReference::new(uuid))
}
fn reference_value(engine: &AppEngine, id: NodeId) -> NodeUuid {
    let ParamValue::Reference(value) = engine.nodes.get(id).unwrap().engine_param_snapshot().unwrap().value else {
        panic!()
    };
    value.uuid
}
fn second(
    engine: &mut AppEngine,
    first: crate::media::protocol::ChannelReference,
) -> crate::media::protocol::ChannelReference {
    let MediaEditResult::Created { reference } = media(
        engine,
        MediaEdit::AddChannel {
            media: first.media,
            label: None,
        },
    ) else {
        panic!()
    };
    reference
}

#[test]
fn media_parameter_selects_the_first_ordered_channel_in_one_undoable_operation() {
    let mut engine = fixture();
    let (_, surface) = create(&mut engine);
    let first = source(&mut engine);
    let second = second(&mut engine, first);
    media(
        &mut engine,
        MediaEdit::ReorderChannel {
            reference: second,
            after: None,
        },
    );
    let (target, channel) = fields(&engine, surface);
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    assert!(set(&mut engine, target, value(first.media.0)).success);
    assert_eq!(definition(&engine, surface).source, Some(second));
    assert_eq!(reference_value(&engine, channel), second.channel.0);
    assert_eq!(engine.undo_len(), history + 1);
    let after = to_sparse_project_json_pretty(&engine).unwrap();
    assert!(engine.undo().unwrap());
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
    assert!(engine.redo().unwrap());
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), after);
    let mut loaded = from_sparse_project_json::<AppNode>(&after).unwrap();
    loaded.apply_edits().unwrap();
    super::super::validate_project(&loaded).unwrap();
    assert_eq!(definition(&loaded, surface).source, Some(second));
}

#[test]
fn explicit_channel_survives_reorder_reselection_and_deletion_without_rerouting() {
    let mut engine = fixture();
    let (_, surface) = create(&mut engine);
    let first = source(&mut engine);
    let second = second(&mut engine, first);
    let (target, channel) = fields(&engine, surface);
    assert!(set(&mut engine, target, value(first.media.0)).success);
    assert!(set(&mut engine, channel, value(second.channel.0)).success);
    media(
        &mut engine,
        MediaEdit::ReorderChannel {
            reference: second,
            after: None,
        },
    );
    assert!(set(&mut engine, target, value(first.media.0)).success);
    assert_eq!(definition(&engine, surface).source, Some(second));
    media(&mut engine, MediaEdit::RemoveChannel { reference: second });
    assert_eq!(definition(&engine, surface).source, Some(second));
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    assert!(set(&mut engine, target, value(first.media.0)).success);
    assert_eq!(engine.undo_len(), history);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
    assert!(engine.undo().unwrap());
    assert_eq!(definition(&engine, surface).source, Some(second));
    assert!(crate::media::channel(&engine, second).is_ok());
}

#[test]
fn explicit_atomic_pair_wins_and_foreign_channel_rejects_without_history() {
    let mut engine = fixture();
    let (_, surface) = create(&mut engine);
    let first = source(&mut engine);
    let second = second(&mut engine, first);
    let foreign = source(&mut engine);
    let (target, channel) = fields(&engine, surface);
    engine.edits.push(Edit::SetParamsAtomic {
        values: vec![(channel, value(second.channel.0)), (target, value(first.media.0))],
        behaviour: ParameterEventBehaviour::Append,
    });
    engine.apply_edits().unwrap();
    engine.dispatch_inbox(ExecutionPhase::EndOfTickStabilization).unwrap();
    assert_eq!(definition(&engine, surface).source, Some(second));
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    assert!(!set(&mut engine, channel, value(foreign.channel.0)).success);
    assert_eq!(engine.undo_len(), history);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
    assert!(set(&mut engine, target, ParamValue::Reference(NodeReference::empty())).success);
    assert!(reference_value(&engine, target).0.is_nil());
    assert!(reference_value(&engine, channel).0.is_nil());
    assert_eq!(definition(&engine, surface).source, None);
}

#[test]
fn reference_targets_choose_an_available_output_and_empty_targets_remain_truthful() {
    let mut engine = fixture();
    let (_, surface) = create(&mut engine);
    let solid = source(&mut engine);
    let MediaEditResult::ReferenceCreated { media: linked } = media(
        &mut engine,
        MediaEdit::CreateReference {
            target: Some(solid.media),
            label: None,
        },
    ) else {
        panic!()
    };
    let MediaEditResult::ReferenceCreated { media: empty } = media(
        &mut engine,
        MediaEdit::CreateReference {
            target: None,
            label: None,
        },
    ) else {
        panic!()
    };
    let (target, channel) = fields(&engine, surface);
    assert!(set(&mut engine, target, value(linked.0)).success);
    assert_eq!(
        definition(&engine, surface).source,
        Some(crate::media::protocol::ChannelReference {
            media: linked,
            channel: crate::media::resolve_source_node(&engine, linked.0).unwrap().channel
        })
    );
    assert!(set(&mut engine, target, value(empty.0)).success);
    assert_eq!(reference_value(&engine, target), empty.0);
    assert_eq!(
        reference_value(&engine, channel),
        crate::media::resolve_source_node(&engine, empty.0).unwrap().channel.0
    );
    assert_eq!(
        definition(&engine, surface).source,
        Some(crate::media::resolve_source_node(&engine, empty.0).unwrap())
    );
    // A raw engine parameter edit uses the same companion policy as transport/Inspector.
    engine.edits.push(Edit::SetParam {
        node: target,
        value: value(solid.media.0),
        behaviour: ParameterEventBehaviour::Append,
    });
    engine.apply_edits().unwrap();
    engine.dispatch_inbox(ExecutionPhase::EndOfTickStabilization).unwrap();
    assert_eq!(definition(&engine, surface).source, Some(solid));
    let matrix = match media(
        &mut engine,
        MediaEdit::CreateMedia {
            media_kind: MediaKind::Matrix,
            label: None,
        },
    ) {
        MediaEditResult::Created { reference } => reference,
        _ => panic!(),
    };
    assert!(set(&mut engine, target, value(matrix.media.0)).success);
    assert_eq!(definition(&engine, surface).source, Some(matrix));
}

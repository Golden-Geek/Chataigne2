use super::*;
use golden_core::{
    node::{Folder, Node},
    parameter::{ParamValue, ParameterEventBehaviour},
    process_ctx::ExecutionPhase,
};

#[test]
fn observation_warning_retries_failed_publication_then_clears_the_actual_applied_warning() {
    let mut engine = crate::app::AppEngine::new(Folder::new("root").into());
    let node = engine.root;
    let mut warning = ObservationWarning::default();
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    warning.service(&mut ctx, node, Some(ObservationCaptureError::Capacity));
    let edits = ctx.edits.pending.len();
    warning.service(&mut ctx, node, None);
    assert_eq!(
        ctx.edits.pending.len(),
        edits,
        "no duplicate while publication is pending"
    );
    engine.edits.push(Edit::SetParam {
        node: NodeId(u64::MAX),
        value: ParamValue::Int(1),
        behaviour: ParameterEventBehaviour::Append,
    });
    engine.absorb_edits(&mut ctx).unwrap();
    assert!(engine.apply_edits().is_err());
    assert_eq!(warning.published, None);
    let mut retry = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    warning.service(&mut retry, node, Some(ObservationCaptureError::Capacity));
    engine.absorb_edits(&mut retry).unwrap();
    engine.apply_edits().unwrap();
    assert!(engine
        .nodes
        .get(node)
        .unwrap()
        .node_data()
        .meta
        .presentation
        .warning(Some(WARNING_ID))
        .is_some());
    let mut recovery = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    warning.service(&mut recovery, node, None);
    assert_eq!(warning.published, Some(Some(ObservationCaptureError::Capacity)));
    assert!(warning.has_pending());
    engine.absorb_edits(&mut recovery).unwrap();
    engine.apply_edits().unwrap();
    let mut settled = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    warning.service(&mut settled, node, None);
    assert!(!warning.has_pending());
    assert!(settled.edits.pending.is_empty());
    assert!(engine
        .nodes
        .get(node)
        .unwrap()
        .node_data()
        .meta
        .presentation
        .warning(Some(WARNING_ID))
        .is_none());
}

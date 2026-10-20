use super::*;
use golden_core::{
    edit::Edit,
    node::Folder,
    parameter::{ParamValue, ParameterEventBehaviour},
    process_ctx::ExecutionPhase,
};

#[test]
fn telemetry_retries_an_unapplied_edit_without_marking_it_published() {
    let mut native = golden_audio::AudioEngineBuilder::default()
        .without_backends()
        .with_backend(golden_audio::NullBackend)
        .build()
        .unwrap();
    let (capture, collector) = super::super::capture::ObservationCollector::new(native.observations()).unwrap();
    let view = capture.latest();
    let mut engine = crate::app::AppEngine::new(Folder::new("root").into());
    let mut delivery = TelemetryDelivery::default();
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    delivery.service(&mut ctx, engine.root, &view);
    delivery.service(&mut ctx, engine.root, &view);
    assert_eq!(ctx.edits.pending.len(), 1);
    engine.edits.push(Edit::SetParam {
        node: NodeId(u64::MAX),
        value: ParamValue::Int(1),
        behaviour: ParameterEventBehaviour::Append,
    });
    engine.absorb_edits(&mut ctx).unwrap();
    assert!(engine.apply_edits().is_err());
    assert!(delivery.published.is_none());
    let mut retry = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    delivery.service(&mut retry, engine.root, &view);
    engine.absorb_edits(&mut retry).unwrap();
    engine.apply_edits().unwrap();
    let mut settled = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    delivery.service(&mut settled, engine.root, &view);
    assert!(!delivery.has_pending());
    assert!(settled.edits.pending.is_empty());
    assert!(delivery.published.is_some());
    drop((delivery, view, collector, capture));
    native.shutdown().unwrap();
}

use super::*;
use golden_core::process_ctx::ProcessCtx;
use std::time::Instant;

#[test]
fn idle_and_early_retry_wakes_stay_passive_and_do_not_capture_a_snapshot() {
    let (mut engine, id) = create_mqtt_module();
    let time = engine.time;
    let crate::app::AppNode::MqttModule(module) = engine.nodes.get_mut(id).unwrap() else {
        unreachable!()
    };
    assert!(!module.needs_update());
    assert!(!module.worker_wake_requires_tree_snapshot());
    module.schedule_retry();
    let at = module.retry_at.unwrap();
    assert!(at > Instant::now());
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, time);
    module.on_worker_wake(&mut ctx);
    assert_eq!(module.retry_at, Some(at));
    assert!(!module.transport_dirty);
    assert!(ctx.tree_snapshot().is_none());
    assert!(!module.worker_wake_requires_tree_snapshot());
    module.retry_at = Some(Instant::now() - Duration::from_secs(1));
    assert!(module.worker_wake_requires_tree_snapshot());
}

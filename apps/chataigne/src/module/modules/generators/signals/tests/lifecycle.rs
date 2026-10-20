use super::*;
use std::time::{Duration, Instant};

fn module(engine: &crate::app::AppEngine, id: NodeId) -> &super::super::SignalsModule {
    match engine.nodes.get(id).unwrap() {
        crate::app::AppNode::SignalsModule(module) => module,
        _ => panic!("expected generator module"),
    }
}

fn drive_until(engine: &mut crate::app::AppEngine, ready: impl Fn(&crate::app::AppEngine) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        engine.run_tick(Duration::from_millis(10)).unwrap();
        if ready(engine) {
            return;
        }
        assert!(Instant::now() < deadline, "generator lifecycle must make progress");
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn passive_generator_recovers_from_worker_exit_without_periodic_polling() {
    let (mut engine, id) = create_signals_module();
    drive_until(&mut engine, |engine| {
        module(engine, id).runtime.is_some() && !module(engine, id).config_dirty
    });
    assert_eq!(
        module(&engine, id).execution_rule(),
        golden_core::engine::NodeExecutionRule::passive()
    );
    assert!(!module(&engine, id).needs_update());
    match engine.nodes.get_mut(id).unwrap() {
        crate::app::AppNode::SignalsModule(module) => module.runtime.as_mut().unwrap().stop(),
        _ => unreachable!(),
    }
    drive_until(&mut engine, |engine| module(engine, id).runtime.is_none());
    assert!(module(&engine, id).config_dirty);
    assert!(module(&engine, id).retry_at.is_some());
    drive_until(&mut engine, |engine| {
        module(engine, id).runtime.is_some() && !module(engine, id).config_dirty
    });
    assert!(module(&engine, id).retry_at.is_none());
}

#[test]
fn disable_reenable_and_destroy_undo_fence_old_worker_lifetimes() {
    let (mut engine, id) = create_signals_module();
    drive_until(&mut engine, |engine| {
        module(engine, id).runtime.is_some() && !module(engine, id).config_dirty
    });
    patch_enabled(&mut engine, id, false);
    stabilize(&mut engine);
    assert!(module(&engine, id).runtime.is_none());
    assert!(module(&engine, id).retry_at.is_none());
    patch_enabled(&mut engine, id, true);
    drive_until(&mut engine, |engine| {
        module(engine, id).runtime.is_some() && !module(engine, id).config_dirty
    });
    let retired = module(&engine, id).wake.as_ref().unwrap().source();
    engine.edits.push(Edit::RemoveNode { node: id });
    engine.apply_edits().unwrap();
    assert!(retired.is_closed());
    assert!(engine.undo().unwrap());
    drive_until(&mut engine, |engine| {
        module(engine, id).runtime.is_some() && !module(engine, id).config_dirty
    });
    assert!(retired.is_closed());
    assert_eq!(retired.wake(), Err(golden_core::engine::NodeWakeError::Closed));
    assert_ne!(module(&engine, id).wake.as_ref().unwrap().source(), retired);
}

#[test]
fn over_limit_authored_label_stops_processing_and_a_valid_edit_recovers() {
    let (mut engine, id) = create_signals_module();
    drive_until(&mut engine, |engine| {
        module(engine, id).runtime.is_some() && !module(engine, id).config_dirty
    });
    let list = find_path(&engine, id, "parameters/signals").unwrap();
    let item = nth_child(&engine, list, 0).unwrap();
    engine.edits.push(Edit::PatchMeta {
        node: item,
        patch: NodeMetaPatch {
            label: Some("x".repeat(1025)),
            ..Default::default()
        },
    });
    engine.apply_edits().unwrap();
    drive_until(&mut engine, |engine| {
        module(engine, id).runtime.is_none() && !module(engine, id).config_dirty
    });
    engine.edits.push(Edit::PatchMeta {
        node: item,
        patch: NodeMetaPatch {
            label: Some("Restored".into()),
            ..Default::default()
        },
    });
    engine.apply_edits().unwrap();
    drive_until(&mut engine, |engine| {
        module(engine, id).runtime.is_some() && !module(engine, id).config_dirty
    });
}

use super::*;
use std::time::{Duration, Instant};

#[test]
fn null_runtime_worker_wakes_resume_startup_and_fence_destroyed_sources_across_undo() {
    let (mut engine, module) = create_sound_card();
    wait_for_null_runtime(&mut engine, module);
    let old_source = engine.node_waker(module).unwrap();
    engine.edits.push(Edit::RemoveNode { node: module });
    engine.apply_edits().unwrap();
    assert_eq!(old_source.wake(), Err(golden_core::engine::NodeWakeError::Closed));
    assert!(engine.undo().unwrap());
    let restored_source = engine.node_waker(module).unwrap();
    assert_ne!(old_source, restored_source);
    wait_for_null_runtime(&mut engine, module);
}

fn wait_for_null_runtime(engine: &mut crate::app::AppEngine, module: NodeId) {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut wake_callbacks = 0;
    loop {
        engine.run_tick(Duration::from_millis(20)).unwrap();
        wake_callbacks += engine.tick_stats().wake_callbacks_fired;
        let crate::app::AppNode::SoundCardModule(node) = engine.nodes.get(module).unwrap() else {
            panic!("Sound Card module")
        };
        assert!(node.execution_rule().update_rate.is_none());
        if let Some(runtime) = node.runtime.as_ref() {
            assert_eq!(runtime.driver().map(BackendId::as_str), Some("null"));
            if node.runtime_request.is_none() && wake_callbacks > 0 {
                return;
            }
        }
        assert!(
            Instant::now() < deadline,
            "reserved wakes must resume the one-shot null worker"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn replacing_a_live_null_runtime_retires_it_and_undo_restarts_with_a_fresh_source() {
    let (mut engine, module) = create_sound_card();
    wait_for_null_runtime(&mut engine, module);
    let old_source = engine.node_waker(module).unwrap();
    let child = engine.nodes.get(module).unwrap().node_data().first_child;
    engine.edits.push(Edit::ReplaceNode {
        node: module,
        new_node: Box::new(golden_core::node::Folder::new("Replacement")),
    });
    engine.apply_edits().unwrap();
    assert_eq!(old_source.wake(), Err(golden_core::engine::NodeWakeError::Closed));
    assert_eq!(engine.nodes.get(module).unwrap().node_data().first_child, child);
    assert!(engine.undo().unwrap());
    wait_for_null_runtime(&mut engine, module);
    let restored_source = engine.node_waker(module).unwrap();
    assert_ne!(old_source, restored_source);
    assert!(engine.redo().unwrap());
    assert_eq!(restored_source.wake(), Err(golden_core::engine::NodeWakeError::Closed));
    assert!(engine.undo().unwrap());
    wait_for_null_runtime(&mut engine, module);
}

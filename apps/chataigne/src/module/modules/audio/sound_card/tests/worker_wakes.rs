use super::super::NO_AUDIO_DRIVER;
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

#[test]
fn disabled_null_module_services_native_completions_without_scheduled_polling_or_a_tree_snapshot() {
    let (mut engine, module) = create_sound_card();
    wait_for_null_runtime(&mut engine, module);
    patch_enabled(&mut engine, module, false);
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut wakes = 0;
    loop {
        engine.run_tick(Duration::from_millis(20)).unwrap();
        wakes += engine.tick_stats().wake_callbacks_fired;
        let crate::app::AppNode::SoundCardModule(node) = engine.nodes.get(module).unwrap() else {
            panic!("Sound Card module")
        };
        assert!(!node.node_data().effective_enabled);
        assert!(!node.worker_wake_requires_tree_snapshot());
        assert!(node.execution_rule().update_rate.is_none());
        if wakes > 0 && node.runtime.is_some() && node.runtime_request.is_none() {
            break;
        }
        assert!(Instant::now() < deadline, "disabled module must service native wakes");
        std::thread::sleep(Duration::from_millis(1));
    }
    patch_enabled(&mut engine, module, true);
    wait_for_null_runtime(&mut engine, module);
}

#[test]
fn clearing_the_driver_releases_the_lifecycle_worker_and_reselection_restarts_it() {
    let (mut engine, module) = create_sound_card();
    for _ in 0..3 {
        wait_for_null_runtime(&mut engine, module);
        let crate::app::AppNode::SoundCardModule(node) = engine.nodes.get(module).unwrap() else {
            panic!("Sound Card")
        };
        let completion = node.runtime.as_ref().unwrap().completion();
        select_enum_parameter(&mut engine, module, "connection/audio_driver", NO_AUDIO_DRIVER);
        wait_for_idle_selection(&mut engine, module);
        assert!(
            completion.wait_for_idle(Duration::ZERO),
            "worker exit must follow native cleanup"
        );
        select_enum_parameter(&mut engine, module, "connection/audio_driver", "null");
    }
    wait_for_null_runtime(&mut engine, module);
}

#[test]
fn clearing_the_driver_while_disabled_still_releases_its_lifecycle_slot() {
    let (mut engine, module) = create_sound_card();
    wait_for_null_runtime(&mut engine, module);
    patch_enabled(&mut engine, module, false);
    select_enum_parameter(&mut engine, module, "connection/audio_driver", NO_AUDIO_DRIVER);
    wait_for_idle_selection(&mut engine, module);
    select_enum_parameter(&mut engine, module, "connection/audio_driver", "null");
    patch_enabled(&mut engine, module, true);
    wait_for_null_runtime(&mut engine, module);
}

fn wait_for_idle_selection(engine: &mut crate::app::AppEngine, module: NodeId) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        engine.run_tick(Duration::from_millis(20)).unwrap();
        let crate::app::AppNode::SoundCardModule(node) = engine.nodes.get(module).unwrap() else {
            panic!("Sound Card")
        };
        assert!(node.runtime.is_none());
        assert!(!node.worker_wake_requires_tree_snapshot());
        if node.runtime_worker.is_none() {
            assert!(node.runtime_request.is_none());
            assert!(node.runtime_retry_at.is_none());
            assert!(node.runtime_error_node.is_none());
            return;
        }
        assert!(
            Instant::now() < deadline,
            "idle selection must release the startup worker"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn full_idle_admission_cannot_strand_a_reselection_of_the_cancelled_driver() {
    use golden_io::RetirementPool;
    use std::{
        num::NonZeroUsize,
        sync::{
            atomic::{AtomicBool, Ordering},
            mpsc,
        },
    };

    let (mut engine, module) = create_sound_card();
    wait_for_null_runtime(&mut engine, module);
    let pool = RetirementPool::new(NonZeroUsize::MIN);
    let (started_tx, started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let first = AtomicBool::new(true);
    let crate::app::AppNode::SoundCardModule(node) = engine.nodes.get_mut(module).unwrap() else {
        panic!("Sound Card")
    };
    node.runtime = None;
    node.runtime_worker = None;
    let mut worker =
        runtime::SoundCardRuntimeWorker::spawn_using(node.runtime_wake.clone().unwrap(), &pool, move |rate, driver| {
            if first.swap(false, Ordering::Relaxed) {
                started_tx.send(()).unwrap();
                let _ = release_rx.recv();
                Err("cancelled first constructor".into())
            } else {
                runtime::SoundCardRuntime::start_selected(rate, driver)
            }
        })
        .unwrap();
    let rate = golden_audio::SampleRate::default();
    let driver = Some(golden_audio::NullBackend::backend_id());
    node.runtime_request = Some(worker.request_start_for_driver(rate, driver.clone()).unwrap());
    started_rx.recv_timeout(Duration::from_secs(3)).unwrap();
    for _ in 0..8 {
        worker.request_start_for_driver(rate, driver.clone()).unwrap();
    }
    node.runtime_worker = Some(worker);
    select_enum_parameter(&mut engine, module, "connection/audio_driver", NO_AUDIO_DRIVER);
    let crate::app::AppNode::SoundCardModule(node) = engine.nodes.get(module).unwrap() else {
        panic!("Sound Card")
    };
    assert!(
        node.runtime_request.is_none(),
        "full idle admission must invalidate the cancelled startup"
    );
    assert!(node.runtime_retry_at.is_some());
    select_enum_parameter(&mut engine, module, "connection/audio_driver", "null");
    release_tx.send(()).unwrap();
    wait_for_null_runtime(&mut engine, module);
    select_enum_parameter(&mut engine, module, "connection/audio_driver", NO_AUDIO_DRIVER);
    wait_for_idle_selection(&mut engine, module);
    assert!(pool.wait_for_idle(Duration::from_secs(3)));
}

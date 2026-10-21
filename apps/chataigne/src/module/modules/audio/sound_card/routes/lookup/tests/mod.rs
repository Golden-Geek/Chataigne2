use super::super::tests::{add_routes, context, fixture, module_mut};
use super::*;
use std::time::{Duration, Instant};

#[test]
fn empty_lookup_is_passive() {
    let lookup = Lookup::default();
    assert!(!lookup.pending());
    assert!(!lookup.needs_snapshot());
}

#[test]
fn worker_matching_preserves_authored_uuid_and_first_duplicate_in_child_order() {
    let (mut engine, module) = fixture();
    let snapshot = engine.process_tree_snapshot();
    let reference = super::super::default_channel_reference(&snapshot, module, "output", 1)
        .unwrap()
        .unwrap();
    let parent = find_path(&snapshot, module, OUTPUT_ROUTES_PATH).unwrap();
    for _ in 0..2 {
        engine.edits.push(Edit::AddNodeTree {
            parent,
            tree: NodeTree::new(SoundCardOutputRoute::connected(reference.clone(), "arbitrary")),
            prev_sibling: None,
        });
    }
    for _ in 0..8 {
        engine.apply_edits().unwrap();
    }
    let snapshot = engine.process_tree_snapshot();
    let first = snapshot.child_ids_slice(parent)[0];
    let result = prepare(
        Job {
            module,
            source: snapshot.try_lease().unwrap(),
            intent: Intent {
                direction: golden_audio::AudioDirection::Output,
                key: "arbitrary".into(),
                channel: reference.uuid(),
                connect: false,
            },
        },
        &golden_io::WorkerCancellation::default(),
    )
    .unwrap();
    assert_eq!(result.found, Some(first));
}

#[test]
fn ordered_disconnects_remove_both_duplicate_occurrences_and_retire_the_worker() {
    let (mut engine, module) = fixture();
    let snapshot = engine.process_tree_snapshot();
    let reference = super::super::default_channel_reference(&snapshot, module, "output", 1)
        .unwrap()
        .unwrap();
    let parent = find_path(&snapshot, module, OUTPUT_ROUTES_PATH).unwrap();
    for _ in 0..2 {
        engine.edits.push(Edit::AddNodeTree {
            parent,
            tree: NodeTree::new(SoundCardOutputRoute::connected(reference.clone(), "duplicate")),
            prev_sibling: None,
        });
    }
    for _ in 0..8 {
        engine.apply_edits().unwrap();
    }
    let snapshot = engine.process_tree_snapshot();
    for _ in 0..2 {
        module_mut(&mut engine, module)
            .enqueue_route_intent(
                &snapshot,
                golden_audio::AudioDirection::Output,
                "duplicate".into(),
                reference.uuid().0.to_string(),
                false,
            )
            .unwrap();
    }
    let until = Instant::now() + Duration::from_secs(5);
    loop {
        let mut ctx = context();
        ctx.set_tree_snapshot(engine.process_tree_snapshot());
        module_mut(&mut engine, module).service_route_intents(&mut ctx);
        engine.edits.pending.extend(ctx.edits.drain());
        engine.apply_edits().unwrap();
        if module_mut(&mut engine, module).routes.lookup.queue.is_empty() {
            break;
        }
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(engine.process_tree_snapshot().child_ids_slice(parent).is_empty());
    assert!(module_mut(&mut engine, module).routes.lookup.worker.is_none());
}

#[test]
fn lookup_byte_and_queue_admission_reject_before_unbounded_input_is_retained() {
    let (mut engine, module) = fixture();
    let snapshot = engine.process_tree_snapshot();
    assert!(module_mut(&mut engine, module)
        .enqueue_route_intent(
            &snapshot,
            golden_audio::AudioDirection::Output,
            "x".repeat(1025),
            "bad".into(),
            false
        )
        .is_err());
    let mut reserved = String::with_capacity(1025);
    reserved.push('k');
    assert!(module_mut(&mut engine, module)
        .enqueue_route_intent(
            &snapshot,
            golden_audio::AudioDirection::Output,
            reserved,
            "bad".into(),
            false
        )
        .is_err());
    let reference = super::super::default_channel_reference(&snapshot, module, "output", 1)
        .unwrap()
        .unwrap();
    for _ in 0..32 {
        module_mut(&mut engine, module)
            .enqueue_route_intent(
                &snapshot,
                golden_audio::AudioDirection::Output,
                "fixed".into(),
                reference.uuid().0.to_string(),
                false,
            )
            .unwrap();
    }
    assert!(module_mut(&mut engine, module)
        .enqueue_route_intent(
            &snapshot,
            golden_audio::AudioDirection::Output,
            "fixed".into(),
            reference.uuid().0.to_string(),
            false
        )
        .is_err());
    assert_eq!(module_mut(&mut engine, module).routes.lookup.queue.len(), 32);
}

#[test]
fn device_change_cancels_queued_matching_before_it_can_edit_another_selection() {
    let (mut engine, module) = fixture();
    let parent = add_routes(&mut engine, module, "output", 1);
    let snapshot = engine.process_tree_snapshot();
    let reference = super::super::default_channel_reference(&snapshot, module, "output", 1)
        .unwrap()
        .unwrap();
    module_mut(&mut engine, module)
        .enqueue_route_intent(
            &snapshot,
            golden_audio::AudioDirection::Output,
            "fixture-0".into(),
            reference.uuid().0.to_string(),
            false,
        )
        .unwrap();
    let device = find_path(&snapshot, module, "connection/output_device").unwrap();
    engine.edits.push(Edit::SetParam {
        node: device,
        value: ParamValue::Enum("none".into()),
        behaviour: golden_core::parameter::ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().unwrap();
    let mut ctx = context();
    ctx.set_tree_snapshot(engine.process_tree_snapshot());
    module_mut(&mut engine, module).service_route_intents(&mut ctx);
    assert!(module_mut(&mut engine, module).routes.lookup.queue.is_empty());
    assert_eq!(engine.process_tree_snapshot().child_ids_slice(parent).len(), 1);
}

#[test]
fn worker_failure_reports_one_intent_and_a_later_intent_recovers() {
    let (mut engine, module) = fixture();
    let parent = add_routes(&mut engine, module, "output", 1);
    let snapshot = engine.process_tree_snapshot();
    let reference = super::super::default_channel_reference(&snapshot, module, "output", 1)
        .unwrap()
        .unwrap();
    for _ in 0..2 {
        module_mut(&mut engine, module)
            .enqueue_route_intent(
                &snapshot,
                golden_audio::AudioDirection::Output,
                "fixture-0".into(),
                reference.uuid().0.to_string(),
                false,
            )
            .unwrap();
    }
    let pool = RetirementPool::new(NonZeroUsize::new(1).unwrap());
    module_mut(&mut engine, module).routes.lookup.worker = Some(
        BoundedWorker::spawn_notified(
            "failed-route-fixture",
            &pool,
            WorkerLimits {
                command_items: 1,
                command_weight: MAIL_BYTES,
                event_items: 1,
                event_weight: MAIL_BYTES,
            },
            WorkerNotifier::new(|| {}),
            |ctx: golden_io::WorkerContext<Job, Result<Prepared, String>>| {
                let _ = ctx.receive();
            },
        )
        .unwrap(),
    );
    let until = Instant::now() + Duration::from_secs(5);
    loop {
        let mut ctx = context();
        ctx.set_tree_snapshot(engine.process_tree_snapshot());
        module_mut(&mut engine, module).service_route_intents(&mut ctx);
        engine.edits.pending.extend(ctx.edits.drain());
        engine.apply_edits().unwrap();
        if module_mut(&mut engine, module).routes.lookup.queue.is_empty() {
            break;
        }
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(engine.process_tree_snapshot().child_ids_slice(parent).is_empty());
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}

#[test]
fn null_ui_connect_disconnect_and_repeated_connect_keep_one_route_per_pair() {
    use chataigne_sound_card_protocol::{SoundCardUiControlRequest, SOUND_CARD_UI_CONTROL_TOPIC};
    let (mut engine, module) = super::super::super::tests::create_sound_card();
    let until = Instant::now() + Duration::from_secs(5);
    loop {
        engine.run_tick(Duration::from_millis(20)).unwrap();
        let node = module_mut(&mut engine, module);
        if node.runtime.is_some()
            && !node.configuration_dirty
            && !node.output_default_routes_pending
            && !node.routes.pruning()
        {
            break;
        }
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(1));
    }
    let snapshot = engine.process_tree_snapshot();
    let parent = find_path(&snapshot, module, OUTPUT_ROUTES_PATH).unwrap();
    let route = snapshot.child_ids_slice(parent)[0];
    let key = find_path(&snapshot, route, "physical_channel")
        .and_then(|id| snapshot.node(id))
        .and_then(|n| n.param_value.as_ref())
        .and_then(ParamValue::as_str)
        .unwrap();
    let reference = super::super::default_channel_reference(&snapshot, module, "output", 1)
        .unwrap()
        .unwrap();
    for (connect, count) in [(false, 1), (true, 2), (true, 2)] {
        let request = if connect {
            SoundCardUiControlRequest::ConnectRoute {
                direction: golden_audio::AudioDirection::Output,
                physical_channel: key.clone(),
                app_channel_uuid: reference.uuid().0.to_string(),
            }
        } else {
            SoundCardUiControlRequest::DisconnectRoute {
                direction: golden_audio::AudioDirection::Output,
                physical_channel: key.clone(),
                app_channel_uuid: reference.uuid().0.to_string(),
            }
        };
        engine.edits.push(Edit::EmitCustomEvent {
            event: golden_core::events::CustomEvent::from_payload(SOUND_CARD_UI_CONTROL_TOPIC, Some(module), &request)
                .unwrap(),
        });
        engine.apply_edits().unwrap();
        let until = Instant::now() + Duration::from_secs(5);
        loop {
            engine.run_tick(Duration::from_millis(20)).unwrap();
            let actual = engine.process_tree_snapshot().child_ids_slice(parent).len();
            let empty = module_mut(&mut engine, module).routes.lookup.queue.is_empty();
            if actual == count && empty {
                break;
            }
            assert!(
                Instant::now() < until,
                "UI intent must finish from worker and edit notifications: {actual}/{count}"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}

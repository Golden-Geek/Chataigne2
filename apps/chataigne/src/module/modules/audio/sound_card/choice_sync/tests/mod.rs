use super::*;
use golden_core::{engine::Engine, node::Folder, process_ctx::ExecutionPhase};

mod publication;

#[test]
fn warning_cache_advances_only_after_core_receipts_and_failure_uses_coarse_retry() {
    let mut engine = Engine::new(Folder::new("root"));
    let module = engine.root;
    let mut choices = ChoiceSync {
        warning: Some("choice preparation is full".into()),
        ..Default::default()
    };
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    choices.service_warning(&mut ctx, module, None);
    assert!(choices.applied_warning.is_none());
    drop(ctx.edits.drain());
    choices.service_warning(&mut ctx, module, None);
    assert!(choices.pending_warning.is_none());
    assert!(choices.warning_retry_at.is_some());
    assert!(choices.applied_warning.is_none());
    assert!(ctx.edits.pending.is_empty());
    choices.warning_retry_at = Some(Instant::now() - RETRY_DELAY);
    choices.service_warning(&mut ctx, module, None);
    engine.edits.pending.extend(ctx.edits.drain());
    engine.apply_edits().unwrap();
    choices.service_warning(&mut ctx, module, None);
    assert_eq!(choices.applied_warning, Some(choices.warning.clone()));
    assert!(engine
        .nodes
        .get(module)
        .unwrap()
        .node_data()
        .meta
        .presentation
        .warnings
        .iter()
        .any(|warning| warning.id == WARNING_ID));
}

#[test]
fn source_invalidation_does_not_release_pending_warning_receipts() {
    let mut choices = ChoiceSync::default();
    let mut ctx = ProcessCtx::new(
        ExecutionPhase::EngineTick,
        golden_core::engine::EngineTime {
            tick: 0,
            micro: 0,
            seq: 0,
        },
    );
    choices.service_warning(&mut ctx, NodeId(1), None);
    choices.invalidate();
    choices.pause();
    assert!(choices.has_pending());
    assert!(choices.pending_warning.is_some());
    assert!(choices.needs_snapshot(7));
}

fn choices_fixture() -> (
    crate::app::AppEngine,
    NodeId,
    runtime::observation::DeviceInventory,
    RuntimeWakeSender,
    golden_io::RevisionSource,
) {
    let (engine, module) = crate::app::module_modules_audio_sound_card::tests::create_sound_card();
    let (inventory, source) =
        runtime::observation::fixture_inventory_with_source(golden_audio::AudioDeviceInspectorState::default(), 7);
    let wake = RuntimeWakeSender::new(engine.node_waker(module).unwrap()).unwrap();
    (engine, module, inventory, wake, source)
}

fn await_ready(choices: &ChoiceSync) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while !choices.request.as_ref().unwrap().is_complete() {
        assert!(Instant::now() < deadline, "native choices did not complete");
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn publication_waits_for_receipts_and_does_not_request_snapshots_while_core_edits_are_pending() {
    let (mut engine, module, inventory, wake, _source) = choices_fixture();
    let mut choices = ChoiceSync {
        applied_warning: Some(None),
        ..Default::default()
    };
    let source = engine.process_tree_snapshot();
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    choices.service(
        &mut ctx,
        module,
        Some(&wake),
        true,
        Some(&source),
        Some(inventory.clone()),
    );
    await_ready(&choices);
    choices.service(&mut ctx, module, Some(&wake), true, Some(&source), Some(inventory));
    assert!(choices.applied_revision.is_none());
    assert!(choices.pending.is_some());
    assert!(!choices.needs_snapshot(7));
    assert!(ctx.edits.pending.len() <= 5);
    engine.edits.pending.extend(ctx.edits.drain());
    engine.apply_edits().unwrap();
    choices.settle(Some(&wake));
    assert_eq!(choices.applied_revision, Some(7));
    assert!(!choices.needs_snapshot(7));
    assert!(choices.needs_snapshot(8));
    choices.invalidate();
    assert!(choices.needs_snapshot(7));
}

#[test]
fn failed_publication_reprepares_from_the_committed_prefix_instead_of_replaying_it() {
    let (mut engine, module, inventory, wake, _source) = choices_fixture();
    let mut choices = ChoiceSync {
        applied_warning: Some(None),
        ..Default::default()
    };
    let source = engine.process_tree_snapshot();
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    choices.service(
        &mut ctx,
        module,
        Some(&wake),
        true,
        Some(&source),
        Some(inventory.clone()),
    );
    await_ready(&choices);
    choices.service(
        &mut ctx,
        module,
        Some(&wake),
        true,
        Some(&source),
        Some(inventory.clone()),
    );
    let mut requests = ctx.edits.drain().into_iter();
    let guard = requests.next().unwrap();
    assert!(matches!(guard.edit, Edit::CheckPreconditions { .. }));
    let first = requests.next().unwrap();
    let Edit::SetParamConstraints { node: first_node, .. } = &first.edit else {
        panic!("constraint patch")
    };
    let first_node = *first_node;
    assert!(requests.len() > 0, "fixture has a rejected suffix");
    engine.edits.pending.extend([guard, first]);
    engine.apply_edits().unwrap();
    drop(requests);
    choices.settle(Some(&wake));
    assert!(choices.applied_revision.is_none());
    assert!(choices.request.is_none());
    assert!(
        !choices.needs_snapshot(7),
        "coarse retry must not request a full snapshot early"
    );
    choices.retry_at = Some(Instant::now() - RETRY_DELAY);
    let fresh = engine.process_tree_snapshot();
    choices.service(
        &mut ctx,
        module,
        Some(&wake),
        true,
        Some(&fresh),
        Some(inventory.clone()),
    );
    await_ready(&choices);
    choices.service(&mut ctx, module, Some(&wake), true, Some(&fresh), Some(inventory));
    assert!(!ctx
        .edits
        .pending
        .iter()
        .any(|request| matches!(&request.edit, Edit::SetParamConstraints { node, .. } if *node == first_node)));
    engine.edits.pending.extend(ctx.edits.drain());
    engine.apply_edits().unwrap();
    choices.settle(Some(&wake));
    assert_eq!(choices.applied_revision, Some(7));
}

#[test]
fn an_earlier_coalesced_wake_rearms_the_choice_retry_without_snapshot_or_admission() {
    let mut engine = Engine::new(Folder::new("root"));
    let module = engine.root;
    let wake = RuntimeWakeSender::new(engine.node_waker(module).unwrap()).unwrap();
    let mut choices = ChoiceSync {
        applied_warning: Some(None),
        ..Default::default()
    };
    choices.failure("busy".into(), true, Some(&wake));
    let deadline = choices.retry_at.unwrap();
    assert!(!choices.needs_snapshot(7));
    wake.wake();
    engine.run_tick(Duration::from_millis(1)).unwrap();
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    choices.service(&mut ctx, module, Some(&wake), true, None, None);
    assert_eq!(choices.retry_at, Some(deadline));
    assert!(choices.request.is_none());
    engine.edits.pending.extend(ctx.edits.drain());
    engine.apply_edits().unwrap();
    // Consume the warning receipt's immediate continuation first.
    engine.run_tick(Duration::from_millis(1)).unwrap();
    choices.service(&mut ctx, module, Some(&wake), true, None, None);
    let limit = deadline + Duration::from_secs(2);
    loop {
        engine.run_tick(Duration::from_millis(1)).unwrap();
        if engine.tick_stats().wake_callbacks_fired > 0 && Instant::now() >= deadline {
            break;
        }
        assert!(Instant::now() < limit, "retry deadline was lost after an earlier wake");
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn permanent_preparation_failure_is_not_acknowledged_or_repeated_without_a_source_change() {
    use golden_core::parameter::{ParamValue, ParameterEventBehaviour};
    let (mut engine, module, inventory, wake, _source) = choices_fixture();
    let parameter = super::super::find_path(&engine.process_tree_snapshot(), module, "connection/sample_rate").unwrap();
    let mut constraints = engine
        .process_tree_snapshot()
        .node(parameter)
        .unwrap()
        .param_constraints
        .clone()
        .unwrap();
    constraints.enum_options = Default::default();
    engine.edits.push(Edit::SetParamConstraints {
        node: parameter,
        constraints,
    });
    engine.edits.push(Edit::SetParam {
        node: parameter,
        value: ParamValue::Enum("x".repeat(1024 * 1024 + 1)),
        behaviour: ParameterEventBehaviour::Append,
    });
    engine.apply_edits().unwrap();
    let source = engine.process_tree_snapshot();
    let mut choices = ChoiceSync {
        applied_warning: Some(None),
        ..Default::default()
    };
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    choices.service(
        &mut ctx,
        module,
        Some(&wake),
        true,
        Some(&source),
        Some(inventory.clone()),
    );
    await_ready(&choices);
    choices.service(&mut ctx, module, Some(&wake), true, Some(&source), Some(inventory));
    assert!(choices.applied_revision.is_none());
    assert_eq!(choices.failed_revision, Some(7));
    assert!(choices.warning.as_ref().unwrap().contains("1 MiB"));
    assert!(!choices.needs_snapshot(7));
    assert!(choices.needs_snapshot(8));
    choices.invalidate();
    assert!(choices.needs_snapshot(7));
}

#[test]
fn unavailable_current_inventory_cancels_a_superseded_ready_result_without_a_snapshot() {
    let (engine, module, inventory, wake, _source) = choices_fixture();
    let source = engine.process_tree_snapshot();
    let mut choices = ChoiceSync {
        applied_warning: Some(None),
        ..Default::default()
    };
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    choices.service(&mut ctx, module, Some(&wake), true, Some(&source), Some(inventory));
    await_ready(&choices);
    assert!(choices.request.is_some());
    choices.service(&mut ctx, module, Some(&wake), true, None, None);
    assert!(
        choices.request.is_none(),
        "stale prepared storage must not stay pinned awaiting a replacement inventory"
    );
    assert!(choices.pending.is_none());
    assert!(choices.applied_revision.is_none());
    assert!(ctx.edits.pending.is_empty());
}

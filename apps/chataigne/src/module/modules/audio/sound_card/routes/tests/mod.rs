use super::*;
use golden_core::{engine::EngineTime, parameter::ParameterEventBehaviour, process_ctx::ExecutionPhase};

pub(super) fn context() -> ProcessCtx {
    ProcessCtx::new(
        ExecutionPhase::EngineTick,
        EngineTime {
            tick: 1,
            micro: 0,
            seq: 0,
        },
    )
}

pub(super) fn fixture() -> (crate::app::AppEngine, NodeId) {
    let (mut engine, module) = super::super::tests::create_sound_card();
    super::super::tests::select_enum_parameter(
        &mut engine,
        module,
        "connection/input_device",
        super::super::SYSTEM_DEFAULT_DEVICE,
    );
    let snapshot = engine.process_tree_snapshot();
    for path in [INPUT_ROUTES_PATH, OUTPUT_ROUTES_PATH] {
        for &node in snapshot.child_ids_slice(find_path(&snapshot, module, path).unwrap()) {
            engine.edits.push(Edit::RemoveNode { node });
        }
    }
    for _ in 0..8 {
        engine.apply_edits().unwrap();
    }
    let node = module_mut(&mut engine, module);
    node.stop_runtime();
    node.routes = RouteWork::default();
    node.input_route_reset_pending = false;
    node.output_route_reset_pending = false;
    node.input_default_routes_pending = false;
    node.output_default_routes_pending = false;
    (engine, module)
}

pub(super) fn module_mut(engine: &mut crate::app::AppEngine, module: NodeId) -> &mut SoundCardModule {
    engine
        .nodes
        .get_mut(module)
        .unwrap()
        .as_any_mut()
        .downcast_mut::<SoundCardModule>()
        .unwrap()
}

pub(super) fn apply(engine: &mut crate::app::AppEngine, ctx: &mut ProcessCtx) {
    engine.edits.pending.extend(ctx.edits.drain());
    engine.apply_edits().unwrap();
}

pub(super) fn keys(prefix: &str) -> Vec<PhysicalChannelKey> {
    (1..=2)
        .map(|index| PhysicalChannelKey::new(format!("{prefix}-{index}")).unwrap())
        .collect()
}

pub(super) fn add_routes(engine: &mut crate::app::AppEngine, module: NodeId, direction: &str, count: usize) -> NodeId {
    let snapshot = engine.process_tree_snapshot();
    let path = if direction == "input" {
        INPUT_ROUTES_PATH
    } else {
        OUTPUT_ROUTES_PATH
    };
    let parent = find_path(&snapshot, module, path).unwrap();
    let reference = default_channel_reference(&snapshot, module, direction, 1)
        .unwrap()
        .unwrap();
    for index in 0..count {
        let key = format!("fixture-{index}");
        let mut tree = if direction == "input" {
            NodeTree::new(SoundCardInputRoute::connected(key.clone(), reference.clone()))
        } else {
            NodeTree::new(SoundCardOutputRoute::connected(reference.clone(), key.clone()))
        };
        structure::set_route_identity(
            tree.node.as_mut(),
            snapshot.node(module).unwrap().uuid,
            direction,
            &key,
            reference.uuid(),
        );
        engine.edits.push(Edit::AddNodeTree {
            parent,
            tree,
            prev_sibling: None,
        });
    }
    for _ in 0..512 {
        engine.apply_edits().unwrap();
        let snapshot = engine.process_tree_snapshot();
        let children = snapshot.child_ids_slice(parent);
        if children.len() == count
            && children
                .last()
                .is_none_or(|id| find_path(&snapshot, *id, "channel").is_some())
        {
            return parent;
        }
    }
    panic!("route fixture must materialize within lifecycle budget");
}

pub(super) fn inject_failure_after(engine: &mut crate::app::AppEngine, ctx: &mut ProcessCtx, before: usize) {
    for (index, request) in ctx.edits.drain().into_iter().enumerate() {
        if index == before {
            engine.edits.push(Edit::SetParam {
                node: NodeId(u64::MAX),
                value: ParamValue::Bool(true),
                behaviour: ParameterEventBehaviour::Coalesce,
            });
        }
        engine.edits.pending.push(request);
    }
    assert!(engine.apply_edits().is_err());
    for _ in 0..4 {
        engine.apply_edits().unwrap();
    }
}

#[test]
fn empty_route_reset_settles_without_emitting_edits() {
    let (mut engine, module) = super::super::tests::create_sound_card();
    let snapshot = engine.process_tree_snapshot();
    let mut ctx = golden_core::process_ctx::ProcessCtx::new(
        golden_core::process_ctx::ExecutionPhase::EngineTick,
        golden_core::engine::EngineTime {
            tick: 1,
            micro: 0,
            seq: 0,
        },
    );
    let node = engine
        .nodes
        .get_mut(module)
        .unwrap()
        .as_any_mut()
        .downcast_mut::<SoundCardModule>()
        .unwrap();
    node.input_route_reset_pending = true;
    node.output_route_reset_pending = true;
    assert!(!node.reset_routes_for_device_selection(&mut ctx, &snapshot).unwrap());
    assert!(!node.input_route_reset_pending && !node.output_route_reset_pending);
    assert!(ctx.edits.pending.is_empty());
}

#[test]
fn reset_has_one_shared_turn_budget_and_settles_only_after_commit() {
    let (mut engine, module) = fixture();
    let input = add_routes(&mut engine, module, "input", 21);
    let output = add_routes(&mut engine, module, "output", 23);
    module_mut(&mut engine, module).input_route_reset_pending = true;
    module_mut(&mut engine, module).output_route_reset_pending = true;
    let mut total = 0;
    for _ in 0..10 {
        let snapshot = engine.process_tree_snapshot();
        let mut ctx = context();
        let node = module_mut(&mut engine, module);
        let busy = node.reset_routes_for_device_selection(&mut ctx, &snapshot).unwrap();
        let removed = ctx
            .edits
            .pending
            .iter()
            .filter(|request| matches!(request.edit, Edit::RemoveNode { .. }))
            .count();
        assert!(removed <= RESET_TURN);
        total += removed;
        if !busy {
            break;
        }
        assert!(node.input_route_reset_pending || node.output_route_reset_pending);
        let mut duplicate = context();
        assert!(node
            .reset_routes_for_device_selection(&mut duplicate, &snapshot)
            .unwrap());
        assert!(
            duplicate.edits.pending.is_empty(),
            "pending receipts prohibit duplicate removals"
        );
        apply(&mut engine, &mut ctx);
    }
    assert_eq!(total, 44);
    assert!(!module_mut(&mut engine, module).input_route_reset_pending);
    assert!(!module_mut(&mut engine, module).output_route_reset_pending);
    let snapshot = engine.process_tree_snapshot();
    assert!(snapshot.child_ids_slice(input).is_empty() && snapshot.child_ids_slice(output).is_empty());
}

#[test]
fn reset_retries_only_the_unapplied_suffix_after_prefix_failure() {
    let (mut engine, module) = fixture();
    let parent = add_routes(&mut engine, module, "output", 32);
    module_mut(&mut engine, module).output_route_reset_pending = true;
    let snapshot = engine.process_tree_snapshot();
    let original = snapshot.child_ids_slice(parent).to_vec();
    let mut ctx = context();
    assert!(module_mut(&mut engine, module)
        .reset_routes_for_device_selection(&mut ctx, &snapshot)
        .unwrap());
    inject_failure_after(&mut engine, &mut ctx, 6); // Fence, five committed removals, failed suffix.
    assert_eq!(engine.process_tree_snapshot().child_ids_slice(parent).len(), 27);
    let snapshot = engine.process_tree_snapshot();
    let mut retry = context();
    assert!(module_mut(&mut engine, module)
        .reset_routes_for_device_selection(&mut retry, &snapshot)
        .unwrap());
    for request in &retry.edits.pending {
        if let Edit::RemoveNode { node } = request.edit {
            assert!(!original[..5].contains(&node));
        }
    }
    apply(&mut engine, &mut retry);
    assert_eq!(engine.process_tree_snapshot().child_ids_slice(parent).len(), 11);
}

#[test]
fn default_seed_retries_a_partial_prefix_without_duplicate_route_ids() {
    let (mut engine, module) = fixture();
    let physical = keys("seed");
    module_mut(&mut engine, module).output_default_routes_pending = true;
    let snapshot = engine.process_tree_snapshot();
    let parent = find_path(&snapshot, module, OUTPUT_ROUTES_PATH).unwrap();
    let mut ctx = context();
    assert!(module_mut(&mut engine, module)
        .seed_default_routes_from_inventory(&mut ctx, &snapshot, &[], &physical)
        .unwrap());
    assert!(module_mut(&mut engine, module).output_default_routes_pending);
    inject_failure_after(&mut engine, &mut ctx, 2); // First tree applied; second did not.
    assert_eq!(engine.process_tree_snapshot().child_ids_slice(parent).len(), 1);
    let snapshot = engine.process_tree_snapshot();
    let mut retry = context();
    assert!(module_mut(&mut engine, module)
        .seed_default_routes_from_inventory(&mut retry, &snapshot, &[], &physical)
        .unwrap());
    assert_eq!(
        retry
            .edits
            .pending
            .iter()
            .filter(|r| matches!(r.edit, Edit::AddNodeTree { .. }))
            .count(),
        1
    );
    apply(&mut engine, &mut retry);
    let snapshot = engine.process_tree_snapshot();
    let mut final_ctx = context();
    assert!(!module_mut(&mut engine, module)
        .seed_default_routes_from_inventory(&mut final_ctx, &snapshot, &[], &physical)
        .unwrap());
    assert!(!module_mut(&mut engine, module).output_default_routes_pending);
    assert_eq!(snapshot.child_ids_slice(parent).len(), 2);
    assert!(final_ctx.edits.pending.is_empty());
}

#[test]
fn accepted_seed_finishes_before_a_later_device_reset_and_new_defaults() {
    let (mut engine, module) = fixture();
    let old = keys("old");
    let new = keys("new");
    module_mut(&mut engine, module).output_default_routes_pending = true;
    let snapshot = engine.process_tree_snapshot();
    let parent = find_path(&snapshot, module, OUTPUT_ROUTES_PATH).unwrap();
    let mut seed = context();
    assert!(module_mut(&mut engine, module)
        .seed_default_routes_from_inventory(&mut seed, &snapshot, &[], &old)
        .unwrap());
    module_mut(&mut engine, module).output_route_reset_pending = true;
    let mut wait = context();
    assert!(module_mut(&mut engine, module)
        .reset_routes_for_device_selection(&mut wait, &snapshot)
        .unwrap());
    assert!(wait.edits.pending.is_empty());
    assert!(module_mut(&mut engine, module)
        .require_route_authoring_ready(golden_audio::AudioDirection::Output)
        .is_err());
    assert!(module_mut(&mut engine, module)
        .require_route_authoring_ready(golden_audio::AudioDirection::Input)
        .is_ok());
    apply(&mut engine, &mut seed);
    let snapshot = engine.process_tree_snapshot();
    let mut reset = context();
    assert!(module_mut(&mut engine, module)
        .reset_routes_for_device_selection(&mut reset, &snapshot)
        .unwrap());
    apply(&mut engine, &mut reset);
    let snapshot = engine.process_tree_snapshot();
    assert!(snapshot.child_ids_slice(parent).is_empty());
    let mut reseed = context();
    assert!(module_mut(&mut engine, module)
        .seed_default_routes_from_inventory(&mut reseed, &snapshot, &[], &new)
        .unwrap());
    apply(&mut engine, &mut reseed);
    let snapshot = engine.process_tree_snapshot();
    let mut done = context();
    assert!(!module_mut(&mut engine, module)
        .seed_default_routes_from_inventory(&mut done, &snapshot, &[], &new)
        .unwrap());
    let actual = snapshot
        .child_ids_slice(parent)
        .iter()
        .map(|id| {
            find_path(&snapshot, *id, "physical_channel")
                .and_then(|id| snapshot.node(id))
                .and_then(|node| node.param_value.as_ref())
                .and_then(ParamValue::as_str)
                .unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(actual, vec!["new-1", "new-2"]);
}

#[test]
fn a_replaced_or_disabled_then_reenabled_module_rejects_queued_old_route_writes() {
    use golden_core::node::{Folder, NodeMetaPatch};
    for replace in [false, true] {
        let (mut engine, module) = fixture();
        let parent = add_routes(&mut engine, module, "output", 3);
        module_mut(&mut engine, module).output_route_reset_pending = true;
        let snapshot = engine.process_tree_snapshot();
        let mut ctx = context();
        assert!(module_mut(&mut engine, module)
            .reset_routes_for_device_selection(&mut ctx, &snapshot)
            .unwrap());
        if replace {
            let mut replacement = Folder::new("Replacement");
            replacement.node_data_mut().meta.uuid = snapshot.node(module).unwrap().uuid;
            engine.edits.push(Edit::ReplaceNode {
                node: module,
                new_node: Box::new(replacement),
            });
            engine.apply_edits().unwrap();
        } else {
            for enabled in [false, true] {
                engine.edits.push(Edit::PatchMeta {
                    node: module,
                    patch: NodeMetaPatch {
                        enabled: Some(enabled),
                        ..NodeMetaPatch::default()
                    },
                });
                engine.apply_edits().unwrap();
            }
        }
        engine.edits.pending.extend(ctx.edits.drain());
        assert!(
            engine.apply_edits().is_err(),
            "the old route lifetime fence must reject its suffix"
        );
        assert_eq!(engine.process_tree_snapshot().child_ids_slice(parent).len(), 3);
    }
}

#[test]
fn large_route_folder_keeps_the_same_first_turn_edit_bound() {
    let (mut engine, module) = fixture();
    let parent = add_routes(&mut engine, module, "output", 4096);
    module_mut(&mut engine, module).output_route_reset_pending = true;
    let snapshot = engine.process_tree_snapshot();
    let mut ctx = context();
    let started = std::time::Instant::now();
    assert!(module_mut(&mut engine, module)
        .reset_routes_for_device_selection(&mut ctx, &snapshot)
        .unwrap());
    let elapsed = started.elapsed();
    assert_eq!(
        ctx.edits
            .pending
            .iter()
            .filter(|r| matches!(r.edit, Edit::RemoveNode { .. }))
            .count(),
        RESET_TURN
    );
    assert_eq!(snapshot.child_ids_slice(parent).len(), 4096);
    eprintln!(
        "4096-route reset preparation: {elapsed:?}; {} edits",
        ctx.edits.pending.len()
    );
    apply(&mut engine, &mut ctx);
    assert_eq!(
        engine.process_tree_snapshot().child_ids_slice(parent).len(),
        4096 - RESET_TURN
    );
}

#[test]
fn default_seed_bounds_key_and_name_bytes_before_any_tree_is_admitted() {
    use golden_core::node::NodeMetaPatch;
    let (mut engine, module) = fixture();
    module_mut(&mut engine, module).output_default_routes_pending = true;
    let oversized = vec![PhysicalChannelKey::new("k".repeat(PHYSICAL_KEY_BYTES + 1)).unwrap()];
    let snapshot = engine.process_tree_snapshot();
    let mut ctx = context();
    assert!(module_mut(&mut engine, module)
        .seed_default_routes_from_inventory(&mut ctx, &snapshot, &[], &oversized)
        .is_err());
    assert!(ctx.edits.pending.is_empty());
    let channel = default_channel_reference(&snapshot, module, "output", 1)
        .unwrap()
        .unwrap()
        .cached_id()
        .unwrap();
    engine.edits.push(Edit::PatchMeta {
        node: channel,
        patch: NodeMetaPatch {
            short_name: Some("n".repeat(CHANNEL_NAME_BYTES + 1)),
            ..NodeMetaPatch::default()
        },
    });
    engine.apply_edits().unwrap();
    let snapshot = engine.process_tree_snapshot();
    assert!(module_mut(&mut engine, module)
        .seed_default_routes_from_inventory(&mut ctx, &snapshot, &[], &keys("bounded"))
        .is_err());
    assert!(ctx.edits.pending.is_empty());
    assert!(module_mut(&mut engine, module).output_default_routes_pending);
}

#[test]
fn a_rejected_direction_does_not_abandon_the_other_directions_accepted_seed() {
    let (mut engine, module) = fixture();
    module_mut(&mut engine, module).input_default_routes_pending = true;
    module_mut(&mut engine, module).output_default_routes_pending = true;
    let input = keys("input");
    let bad_output = vec![PhysicalChannelKey::new("k".repeat(PHYSICAL_KEY_BYTES + 1)).unwrap()];
    let snapshot = engine.process_tree_snapshot();
    let mut ctx = context();
    assert!(module_mut(&mut engine, module)
        .seed_default_routes_from_inventory(&mut ctx, &snapshot, &input, &bad_output)
        .unwrap());
    apply(&mut engine, &mut ctx);
    let snapshot = engine.process_tree_snapshot();
    let mut ack = context();
    assert!(module_mut(&mut engine, module)
        .seed_default_routes_from_inventory(&mut ack, &snapshot, &input, &bad_output)
        .is_err());
    assert!(!module_mut(&mut engine, module).input_default_routes_pending);
    assert!(module_mut(&mut engine, module).output_default_routes_pending);
    assert!(ack.edits.pending.is_empty());
}

#[test]
fn null_device_disconnect_finishes_route_reset_through_passive_wakes() {
    use std::time::{Duration, Instant};
    let (mut engine, module) = super::super::tests::create_sound_card();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        engine.run_tick(Duration::from_millis(20)).unwrap();
        let node = module_mut(&mut engine, module);
        assert!(node.execution_rule().update_rate.is_none());
        if node.runtime.is_some() && !node.output_default_routes_pending {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "Null defaults must commit via reserved wakes"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    let snapshot = engine.process_tree_snapshot();
    let device = find_path(&snapshot, module, "connection/output_device").unwrap();
    let parent = find_path(&snapshot, module, OUTPUT_ROUTES_PATH).unwrap();
    assert_eq!(snapshot.child_ids_slice(parent).len(), 2);
    let channel = default_channel_reference(&snapshot, module, "output", 1)
        .unwrap()
        .unwrap()
        .cached_id()
        .unwrap();
    engine.edits.push(Edit::PatchMeta {
        node: channel,
        patch: golden_core::node::NodeMetaPatch {
            short_name: Some("Metadata Name".into()),
            ..golden_core::node::NodeMetaPatch::default()
        },
    });
    engine.apply_edits().unwrap();
    assert_ne!(
        engine.process_tree_snapshot().node(channel).unwrap().short_name,
        "Output 1"
    );
    engine.run_tick(Duration::from_millis(20)).unwrap();
    assert_eq!(
        engine.process_tree_snapshot().node(channel).unwrap().short_name,
        "Output 1",
        "short-name-only edits must reconcile to the authored channel label without waiting for native telemetry"
    );
    engine.edits.push(Edit::SetParam {
        node: device,
        value: ParamValue::Enum(super::super::NO_AUDIO_DEVICE.into()),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        engine.run_tick(Duration::from_millis(20)).unwrap();
        let empty = engine.process_tree_snapshot().child_ids_slice(parent).is_empty();
        let node = module_mut(&mut engine, module);
        if empty && !node.output_route_reset_pending && !node.configuration_dirty {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "reset completion must not depend on a later native event"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
}

use super::*;
use golden_core::{edit::NodeTree, process_ctx::ExecutionPhase};

fn module(engine: &crate::app::AppEngine, id: NodeId) -> &MidiModule {
    let crate::app::AppNode::MidiModule(m) = engine.nodes.get(id).unwrap() else {
        panic!("MIDI module")
    };
    m
}
fn module_mut(engine: &mut crate::app::AppEngine, id: NodeId) -> &mut MidiModule {
    let crate::app::AppNode::MidiModule(m) = engine.nodes.get_mut(id).unwrap() else {
        panic!("MIDI module")
    };
    m
}
fn service(engine: &mut crate::app::AppEngine, id: NodeId) -> ProcessCtx {
    let snapshot = engine.process_tree_snapshot();
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    module_mut(engine, id).service_structure(&mut ctx, Some(&snapshot));
    ctx
}
fn apply(engine: &mut crate::app::AppEngine, ctx: &mut ProcessCtx) {
    engine.absorb_edits(ctx).unwrap();
    engine.apply_edits().unwrap();
    engine.dispatch_inbox(ExecutionPhase::EngineTick).unwrap();
    engine.apply_edits().unwrap();
}
fn drain(engine: &mut crate::app::AppEngine, id: NodeId) {
    for _ in 0..300 {
        engine.dispatch_inbox(ExecutionPhase::EngineTick).unwrap();
        engine.apply_edits().unwrap();
        if !module(engine, id).structure_work.busy()
            && engine.edits.pending.is_empty()
            && engine.inbox.events.is_empty()
        {
            return;
        }
        let mut ctx = service(engine, id);
        assert!(ctx.edits.pending.len() <= 224);
        apply(engine, &mut ctx);
    }
    panic!("structure work did not settle")
}
fn fixture(count: u8) -> (crate::app::AppEngine, NodeId, NodeId) {
    let mut engine = crate::app::AppEngine::new(Folder::new("root").into());
    engine.add_node(MidiModule::create().into(), None);
    for _ in 0..6 {
        engine.apply_edits().unwrap();
    }
    let id = engine.nodes.get(engine.root).unwrap().node_data().first_child.unwrap();
    drain(&mut engine, id);
    let values = module(&engine, id).base.values_id().unwrap();
    let mut cc = NodeTree::new(create_folder("CC", CONTROL_CHANGE_FOLDER_DECL_ID));
    for controller in 0..count {
        cc.push_child(
            NodeTree::new(create_int_parameter(
                &cc_label(controller),
                &cc_decl_id(controller),
                i32::from(controller % 32 + 1),
                Some(MIDI_7_BIT_VALUE_RANGE),
                false,
            ))
            .with_child(NodeTree::new(create_cc_rotary_mode_parameter(ROTARY_ABSOLUTE))),
        );
    }
    engine.edits.push(Edit::AddNodeTree {
        parent: values,
        prev_sibling: None,
        tree: NodeTree::new(create_folder("Channel 1", "channel_1")).with_child(cc),
    });
    engine.apply_edits().unwrap();
    let snapshot = engine.process_tree_snapshot();
    let folder = module(&engine, id).find_cc_folder(&snapshot, 1).unwrap();
    (engine, id, folder)
}
fn cc(engine: &crate::app::AppEngine, folder: NodeId, controller: u8) -> Option<NodeId> {
    engine
        .process_tree_snapshot()
        .find_child_by_decl_id(folder, &cc_decl_id(controller))
}
fn value(engine: &crate::app::AppEngine, id: NodeId) -> i32 {
    engine
        .nodes
        .get(id)
        .unwrap()
        .engine_param_snapshot()
        .unwrap()
        .value
        .as_int()
        .unwrap()
}
fn write(engine: &mut crate::app::AppEngine, node: NodeId, value: ParamValue) {
    engine.edits.push(Edit::SetParam {
        node,
        value,
        behaviour: ParameterEventBehaviour::Append,
    });
}
fn width(engine: &mut crate::app::AppEngine, id: NodeId, wide: bool) {
    let toggle = module(engine, id).channel_toggle_id(1).unwrap();
    write(engine, toggle, ParamValue::Bool(wide));
    engine.apply_edits().unwrap();
    engine.dispatch_inbox(ExecutionPhase::EngineTick).unwrap();
}

#[test]
fn all_thirty_two_pairs_convert_in_bounded_plans_with_one_order_phase() {
    let (mut engine, id, folder) = fixture(64);
    let ids = (0..32).map(|c| cc(&engine, folder, c).unwrap()).collect::<Vec<_>>();
    width(&mut engine, id, true);
    let mut ctx = service(&mut engine, id);
    assert!(ctx.edits.pending.len() <= 200);
    assert!(ctx.edits.pending.iter().all(|r| match &r.edit {
        Edit::CallNodeMutation {
            node,
            needs_tree_snapshot,
            ..
        } => (ids.contains(node) && !needs_tree_snapshot) || (*node == id && *needs_tree_snapshot),
        Edit::MoveNode { .. } => false,
        _ => true,
    }));
    apply(&mut engine, &mut ctx);
    drain(&mut engine, id);
    for c in 0..32u8 {
        assert_eq!(cc(&engine, folder, c), Some(ids[usize::from(c)]));
        assert_eq!(value(&engine, ids[usize::from(c)]), i32::from(c + 1) * 129);
        assert!(cc(&engine, folder, c + 32).is_none());
    }
    width(&mut engine, id, false);
    let mut ctx = service(&mut engine, id);
    let trees = ctx
        .edits
        .pending
        .iter()
        .filter_map(|r| {
            if let Edit::AddNodeTree { tree, .. } = &r.edit {
                Some(tree)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(trees.len(), 32);
    assert!(trees.iter().all(|t| t.children.len() == 1));
    assert!(ctx.edits.pending.len() <= 200);
    apply(&mut engine, &mut ctx);
    drain(&mut engine, id);
    for c in 0..32u8 {
        assert_eq!(value(&engine, ids[usize::from(c)]), i32::from(c + 1));
        let low = cc(&engine, folder, c + 32).unwrap();
        assert_eq!(value(&engine, low), i32::from(c + 1));
        assert!(engine
            .process_tree_snapshot()
            .find_child_by_decl_id(low, MIDI_CC_ROTARY_MODE_DECL_ID)
            .is_some());
    }
    let snapshot = engine.process_tree_snapshot();
    let keys = snapshot
        .child_ids_slice(folder)
        .iter()
        .map(|id| cc_from_decl_id(&snapshot.node(*id).unwrap().decl_id).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(keys, (0..64).collect::<Vec<_>>());
}

#[test]
fn unchanged_final_width_coalesces_rapid_toggles_without_converting_values() {
    let (mut engine, id, folder) = fixture(64);
    let toggle = module(&engine, id).channel_toggle_id(1).unwrap();
    write(&mut engine, toggle, ParamValue::Bool(true));
    write(&mut engine, toggle, ParamValue::Bool(false));
    engine.apply_edits().unwrap();
    engine.dispatch_inbox(ExecutionPhase::EngineTick).unwrap();
    engine.apply_edits().unwrap();
    drain(&mut engine, id);
    for c in 0..64u8 {
        assert_eq!(value(&engine, cc(&engine, folder, c).unwrap()), i32::from(c % 32 + 1));
    }
}

#[test]
fn a_discarded_structure_callback_cannot_strand_accepted_work() {
    let (mut engine, id, folder) = fixture(64);
    width(&mut engine, id, true);
    assert!(!engine.edits.pending.is_empty());
    engine.edits.pending.clear();
    drain(&mut engine, id);
    assert!(!module(&engine, id).structure_work.busy());
    for c in 0..32u8 {
        assert_eq!(value(&engine, cc(&engine, folder, c).unwrap()), i32::from(c + 1) * 129);
        assert!(cc(&engine, folder, c + 32).is_none());
    }
}

#[test]
fn pending_edit_receipt_requests_no_snapshot_and_repeats_no_work() {
    let (mut engine, id, _) = fixture(64);
    width(&mut engine, id, true);
    let mut first = service(&mut engine, id);
    assert!(!first.edits.pending.is_empty());
    assert!(!module(&engine, id).structure_work.requires_snapshot());
    for _ in 0..10 {
        let ctx = service(&mut engine, id);
        assert!(ctx.edits.pending.is_empty());
    }
    apply(&mut engine, &mut first);
    drain(&mut engine, id);
}

#[test]
fn stale_read_rejects_before_effects_and_restores_the_unapplied_width() {
    let (mut engine, id, folder) = fixture(64);
    let base = cc(&engine, folder, 0).unwrap();
    width(&mut engine, id, true);
    let mut ctx = service(&mut engine, id);
    write(&mut engine, base, ParamValue::Int(55));
    engine.absorb_edits(&mut ctx).unwrap();
    assert!(engine.apply_edits().is_err());
    drain(&mut engine, id);
    assert!(!module(&engine, id).cc_channel_is_14_bit_handle(1));
    assert_eq!(value(&engine, base), 55);
    assert!(cc(&engine, folder, 32).is_some());
}

#[test]
fn large_retirement_is_rejected_before_cc_values_or_children_change() {
    let (mut engine, id, folder) = fixture(64);
    let low = cc(&engine, folder, 32).unwrap();
    let mut tree = NodeTree::new(Folder::new("custom control"));
    for n in 0..256 {
        tree.push_child(NodeTree::new(Folder::new(format!("child {n}"))));
    }
    engine.edits.push(Edit::AddNodeTree {
        parent: low,
        prev_sibling: None,
        tree,
    });
    engine.apply_edits().unwrap();
    width(&mut engine, id, true);
    engine.apply_edits().unwrap();
    drain(&mut engine, id);
    assert!(!module(&engine, id).cc_channel_is_14_bit_handle(1));
    assert_eq!(cc(&engine, folder, 32), Some(low));
    assert_eq!(value(&engine, cc(&engine, folder, 0).unwrap()), 1);
}

#[test]
fn startup_reconciliation_is_idempotent_for_an_existing_fourteen_bit_value() {
    let (mut engine, id, folder) = fixture(64);
    width(&mut engine, id, true);
    engine.apply_edits().unwrap();
    drain(&mut engine, id);
    let base = cc(&engine, folder, 0).unwrap();
    write(&mut engine, base, ParamValue::Int(32));
    engine.apply_edits().unwrap();
    engine.dispatch_inbox(ExecutionPhase::EngineTick).unwrap();
    engine.apply_edits().unwrap();
    let snapshot = engine.process_tree_snapshot();
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    module_mut(&mut engine, id).start_value_locks(&mut ctx, &snapshot);
    apply(&mut engine, &mut ctx);
    drain(&mut engine, id);
    assert_eq!(value(&engine, base), 32);
    assert!(cc(&engine, folder, 32).is_none());
}

#[test]
fn value_locking_has_small_turns_and_preserves_control_owned_metadata() {
    let (mut engine, id, folder) = fixture(128);
    let cc0 = cc(&engine, folder, 0).unwrap();
    engine.add_node(Folder::new("control owned").into(), Some(cc0));
    engine.apply_edits().unwrap();
    let control = engine.nodes.get(cc0).unwrap().node_data().last_child.unwrap();
    let before = engine
        .nodes
        .get(control)
        .unwrap()
        .node_data()
        .meta
        .user_permissions
        .clone();
    assert_ne!(before, NodeUserPermissions::none());
    let snapshot = engine.process_tree_snapshot();
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    module_mut(&mut engine, id).start_value_locks(&mut ctx, &snapshot);
    let mut turns = 0;
    loop {
        let patches = ctx
            .edits
            .pending
            .iter()
            .filter(|r| matches!(r.edit, Edit::PatchMeta { .. }))
            .count();
        assert!(patches <= 24);
        assert!(ctx.edits.pending.len() <= 224);
        if patches != 0 {
            assert!(ctx.edits.pending.len() <= 25);
        }
        apply(&mut engine, &mut ctx);
        turns += 1;
        if !module(&engine, id).structure_work.busy() {
            break;
        }
        ctx = service(&mut engine, id);
        assert!(turns < 100);
    }
    assert!(turns > 5);
    assert_eq!(
        engine.nodes.get(control).unwrap().node_data().meta.user_permissions,
        before
    );
    for c in 0..128u8 {
        assert_eq!(
            engine
                .nodes
                .get(cc(&engine, folder, c).unwrap())
                .unwrap()
                .node_data()
                .meta
                .user_permissions,
            NodeUserPermissions::none()
        );
    }
}

#[test]
fn accepted_structure_finishes_through_worker_wakes_while_disabled() {
    let (mut engine, id, folder) = fixture(64);
    let toggle = module(&engine, id).channel_toggle_id(1).unwrap();
    engine.edits.push(Edit::PatchMeta {
        node: id,
        patch: NodeMetaPatch {
            enabled: Some(false),
            ..Default::default()
        },
    });
    engine.apply_edits().unwrap();
    engine.dispatch_inbox(ExecutionPhase::EngineTick).unwrap();
    engine.apply_edits().unwrap();
    write(&mut engine, toggle, ParamValue::Bool(true));
    engine.apply_edits().unwrap();
    engine.dispatch_inbox(ExecutionPhase::EngineTick).unwrap();
    engine.apply_edits().unwrap();
    engine.resolve().unwrap();
    for _ in 0..12 {
        if !module(&engine, id).structure_work.busy() {
            break;
        }
        engine.run_tick(Duration::ZERO).unwrap();
        engine.apply_edits().unwrap();
        engine.dispatch_inbox(ExecutionPhase::EngineTick).unwrap();
        engine.apply_edits().unwrap();
        engine.resolve().unwrap();
    }
    assert!(!module(&engine, id).structure_work.busy());
    assert!(cc(&engine, folder, 32).is_none());
    assert!(module(&engine, id).input.is_none());
    assert!(module(&engine, id).output.is_none());
}

#[test]
fn numeric_ordering_is_a_single_bounded_pass() {
    let (mut engine, id, folder) = fixture(128);
    width(&mut engine, id, true);
    engine.apply_edits().unwrap();
    drain(&mut engine, id);
    let last = cc(&engine, folder, 127).unwrap();
    engine.edits.push(Edit::MoveNode {
        node: last,
        new_parent: folder,
        new_prev_sibling: None,
    });
    engine.apply_edits().unwrap();
    let snapshot = engine.process_tree_snapshot();
    let uuid = snapshot.node(folder).unwrap().uuid;
    let mut ordering = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    ordering.edits = cc::order(&snapshot, folder, uuid).unwrap();
    assert!(!ordering.edits.pending.is_empty());
    assert!(ordering.edits.pending.len() <= 129);
    assert!(ordering
        .edits
        .pending
        .iter()
        .all(|r| matches!(r.edit, Edit::MoveNode { .. } | Edit::CheckPreconditions { .. })));
    apply(&mut engine, &mut ordering);
    let snapshot = engine.process_tree_snapshot();
    assert!(cc::order(&snapshot, folder, uuid).unwrap().pending.is_empty());
    let keys = snapshot
        .child_ids_slice(folder)
        .iter()
        .map(|id| cc_from_decl_id(&snapshot.node(*id).unwrap().decl_id).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(keys, (0..32).chain(64..128).collect::<Vec<_>>());
}

#[test]
fn a_failed_suffix_reconciles_completed_widths_without_double_conversion() {
    let (mut engine, id, folder) = fixture(64);
    width(&mut engine, id, true);
    let mut ctx = service(&mut engine, id);
    let first = ctx
        .edits
        .pending
        .iter()
        .position(|r| matches!(r.edit, Edit::CallNodeMutation { .. }))
        .unwrap();
    let suffix = ctx.edits.pending.split_off(first + 1);
    ctx.edits.push(Edit::SetParam {
        node: NodeId(u64::MAX),
        value: ParamValue::Int(1),
        behaviour: ParameterEventBehaviour::Append,
    });
    ctx.edits.pending.extend(suffix);
    engine.absorb_edits(&mut ctx).unwrap();
    assert!(engine.apply_edits().is_err());
    let base = cc(&engine, folder, 0).unwrap();
    assert_eq!(value(&engine, base), 129);
    assert_eq!(
        engine
            .nodes
            .get(base)
            .unwrap()
            .engine_param_snapshot()
            .unwrap()
            .constraints
            .range,
        RangeConstraint::uniform(Some(0.0), Some(16383.0))
    );
    drain(&mut engine, id);
    for c in 0..32u8 {
        assert_eq!(value(&engine, cc(&engine, folder, c).unwrap()), i32::from(c + 1) * 129);
        assert!(cc(&engine, folder, c + 32).is_none());
    }
}

#[test]
fn a_failed_narrowing_suffix_preserves_low_bits_before_reconciliation() {
    let (mut engine, id, folder) = fixture(64);
    width(&mut engine, id, true);
    drain(&mut engine, id);
    let base = cc(&engine, folder, 0).unwrap();
    write(&mut engine, base, ParamValue::Int(300));
    engine.apply_edits().unwrap();
    width(&mut engine, id, false);
    let mut ctx = service(&mut engine, id);
    let first = ctx
        .edits
        .pending
        .iter()
        .position(|r| matches!(r.edit, Edit::CallNodeMutation { .. }))
        .unwrap();
    let suffix = ctx.edits.pending.split_off(first + 1);
    ctx.edits.push(Edit::SetParam {
        node: NodeId(u64::MAX),
        value: ParamValue::Int(1),
        behaviour: ParameterEventBehaviour::Append,
    });
    ctx.edits.pending.extend(suffix);
    engine.absorb_edits(&mut ctx).unwrap();
    assert!(engine.apply_edits().is_err());
    assert_eq!(value(&engine, base), 2);
    assert_eq!(value(&engine, cc(&engine, folder, 32).unwrap()), 44);
    drain(&mut engine, id);
    assert_eq!(value(&engine, base), 2);
    assert_eq!(value(&engine, cc(&engine, folder, 32).unwrap()), 44);
    for c in 1..32u8 {
        assert_eq!(value(&engine, cc(&engine, folder, c).unwrap()), i32::from(c + 1));
        assert_eq!(value(&engine, cc(&engine, folder, c + 32).unwrap()), i32::from(c + 1));
    }
}

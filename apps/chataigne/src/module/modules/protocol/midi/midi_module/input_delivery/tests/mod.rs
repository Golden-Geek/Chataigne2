use super::*;
use golden_core::{events::EventKind, node::Folder, parameter::ParameterChangeCheck, process_ctx::ExecutionPhase};

fn mode() -> Mode {
    Mode {
        bits: 0,
        auto_add: true,
        precision: 2,
    }
}
fn input() -> RetainedPayload<PreparedInput> {
    midi_input::InputDraft::prepared(MidiMessage::NoteOn {
        channel: 1,
        note: 60,
        velocity: 1,
    })
}
fn fixture() -> (crate::app::AppEngine, NodeId) {
    let mut engine = crate::app::AppEngine::new(Folder::new("values").into());
    engine.add_node(
        Parameter::new("pulse", ParamValue::Trigger(), ParameterChangeCheck::None).into(),
        None,
    );
    engine.apply_edits().unwrap();
    let id = engine.nodes.get(engine.root).unwrap().node_data().first_child.unwrap();
    engine.inbox.events.clear();
    (engine, id)
}
fn service(engine: &mut crate::app::AppEngine, delivery: &mut InputDelivery) -> (bool, ProcessCtx) {
    let snapshot = engine.process_tree_snapshot();
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    let done = delivery.service(
        &mut ctx,
        Some(&snapshot),
        Some(engine.root),
        mode(),
        &mut HashMap::new(),
    );
    (done, ctx)
}
fn apply(engine: &mut crate::app::AppEngine, ctx: &mut ProcessCtx) {
    engine.absorb_edits(ctx).unwrap();
    engine.apply_edits().unwrap();
}
fn settle(engine: &mut crate::app::AppEngine, delivery: &mut InputDelivery) {
    for _ in 0..12 {
        let (done, mut ctx) = service(engine, delivery);
        apply(engine, &mut ctx);
        if done {
            return;
        }
    }
    panic!("input did not settle");
}
fn callbacks(engine: &crate::app::AppEngine) -> usize {
    engine.inbox.events.iter().filter(|e| matches!(&e.kind, EventKind::Custom(c) if c.topic == crate::app::module::script_api::MODULE_SCRIPT_CALLBACK_TOPIC)).count()
}
fn pulses(engine: &crate::app::AppEngine, id: NodeId) -> usize {
    engine
        .inbox
        .events
        .iter()
        .filter(|e| matches!(e.kind, EventKind::ParamChanged { param, .. } if param == id))
        .count()
}
fn path(value: ParamValue) -> Write {
    Write {
        target: Target::Path(vec![
            FolderSpec {
                label: "Channel 1".into(),
                decl: "channel_1".into(),
            },
            FolderSpec {
                label: "Notes".into(),
                decl: NOTES_FOLDER_DECL_ID.into(),
            },
        ]),
        value: Value::Small(value),
        label: "Note 60".into(),
        decl: note_decl_id(60),
        range: None,
        read_only: false,
        rotary: None,
        initialized: false,
    }
}

#[test]
fn missing_subtree_is_one_edit_and_does_not_replay_an_earlier_trigger() {
    let (mut engine, pulse) = fixture();
    let mut delivery = InputDelivery::new(
        input(),
        vec![Write::direct(pulse, ParamValue::Trigger()), path(ParamValue::Int(42))],
        engine.root,
        mode(),
    );
    let (_, mut ctx) = service(&mut engine, &mut delivery);
    assert_eq!(
        ctx.edits
            .pending
            .iter()
            .filter(|e| matches!(e.edit, Edit::AddNodeTree { .. }))
            .count(),
        1
    );
    assert_eq!(ctx.edits.pending.len(), 2);
    apply(&mut engine, &mut ctx);
    settle(&mut engine, &mut delivery);
    assert_eq!(pulses(&engine, pulse), 1);
    assert_eq!(callbacks(&engine), 2);
    let snapshot = engine.process_tree_snapshot();
    let channel = snapshot.find_child_by_decl_id(engine.root, "channel_1").unwrap();
    let notes = snapshot.find_child_by_decl_id(channel, NOTES_FOLDER_DECL_ID).unwrap();
    let note = snapshot.find_child_by_decl_id(notes, &note_decl_id(60)).unwrap();
    assert_eq!(snapshot.node(note).unwrap().param_value, Some(ParamValue::Int(42)));
    assert_eq!(
        pulses(&engine, note),
        0,
        "creation initializes a scalar without a duplicate write"
    );
}

#[test]
fn failed_graph_receipt_abandons_suffix_and_never_replays_applied_prefix() {
    let (mut engine, pulse) = fixture();
    let mut delivery = InputDelivery::new(
        input(),
        vec![Write::direct(pulse, ParamValue::Trigger()), path(ParamValue::Int(42))],
        engine.root,
        mode(),
    );
    let (_, mut ctx) = service(&mut engine, &mut delivery);
    let last = ctx.edits.pending.pop().unwrap();
    ctx.edits.push(Edit::SetParam {
        node: NodeId(u64::MAX),
        value: ParamValue::Int(0),
        behaviour: ParameterEventBehaviour::Append,
    });
    ctx.edits.pending.push(last);
    engine.absorb_edits(&mut ctx).unwrap();
    assert!(engine.apply_edits().is_err());
    settle(&mut engine, &mut delivery);
    assert_eq!(pulses(&engine, pulse), 1);
    assert_eq!(callbacks(&engine), 2);
    assert!(engine
        .process_tree_snapshot()
        .find_child_by_decl_id(engine.root, "channel_1")
        .is_none());
}

#[test]
fn callback_failure_retries_only_the_unpublished_specific_callback() {
    let (mut engine, _) = fixture();
    let mut delivery = InputDelivery::new(input(), vec![], engine.root, mode());
    let (_, mut ctx) = service(&mut engine, &mut delivery);
    assert_eq!(ctx.edits.pending.len(), 2);
    let specific = ctx.edits.pending.pop().unwrap();
    ctx.edits.push(Edit::SetParam {
        node: NodeId(u64::MAX),
        value: ParamValue::Int(0),
        behaviour: ParameterEventBehaviour::Append,
    });
    ctx.edits.pending.push(specific);
    engine.absorb_edits(&mut ctx).unwrap();
    assert!(engine.apply_edits().is_err());
    assert_eq!(callbacks(&engine), 1);
    settle(&mut engine, &mut delivery);
    assert_eq!(callbacks(&engine), 2);
}

#[test]
fn pending_receipt_emits_no_new_work_or_duplicate_occurrences() {
    let (mut engine, pulse) = fixture();
    let mut delivery = InputDelivery::new(
        input(),
        vec![Write::direct(pulse, ParamValue::Trigger())],
        engine.root,
        mode(),
    );
    let (_, mut first) = service(&mut engine, &mut delivery);
    for _ in 0..10 {
        let (done, ctx) = service(&mut engine, &mut delivery);
        assert!(!done);
        assert!(ctx.edits.pending.is_empty());
    }
    apply(&mut engine, &mut first);
    settle(&mut engine, &mut delivery);
    assert_eq!(pulses(&engine, pulse), 1);
    assert_eq!(callbacks(&engine), 2);
}

#[test]
fn disabling_auto_add_fences_unapplied_suffix_but_retains_callbacks() {
    let (mut engine, pulse) = fixture();
    let mut delivery = InputDelivery::new(
        input(),
        vec![Write::direct(pulse, ParamValue::Trigger()), path(ParamValue::Int(42))],
        engine.root,
        mode(),
    );
    let (_, mut ctx) = service(&mut engine, &mut delivery);
    apply(&mut engine, &mut ctx);
    let snapshot = engine.process_tree_snapshot();
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    let changed = Mode {
        auto_add: false,
        ..mode()
    };
    delivery.service(
        &mut ctx,
        Some(&snapshot),
        Some(engine.root),
        changed,
        &mut HashMap::new(),
    );
    assert!(ctx
        .edits
        .pending
        .iter()
        .all(|e| matches!(e.edit, Edit::EmitCustomEvent { .. })));
    apply(&mut engine, &mut ctx);
    settle(&mut engine, &mut delivery);
    assert_eq!(pulses(&engine, pulse), 1);
    assert_eq!(callbacks(&engine), 2);
}

#[test]
fn a_new_trigger_leaf_fires_once_after_creation() {
    let (mut engine, _) = fixture();
    let mut delivery = InputDelivery::new(input(), vec![path(ParamValue::Trigger())], engine.root, mode());
    settle(&mut engine, &mut delivery);
    assert_eq!(
        engine
            .inbox
            .events
            .iter()
            .filter(|e| matches!(e.kind, EventKind::ParamChanged { .. }))
            .count(),
        1
    );
    assert_eq!(callbacks(&engine), 2);
}

#[test]
fn repeated_messages_keep_each_trigger_and_both_callback_occurrences() {
    let (mut engine, pulse) = fixture();
    for _ in 0..2 {
        let mut delivery = InputDelivery::new(
            input(),
            vec![Write::direct(pulse, ParamValue::Trigger())],
            engine.root,
            mode(),
        );
        settle(&mut engine, &mut delivery);
    }
    assert_eq!(pulses(&engine, pulse), 2);
    assert_eq!(callbacks(&engine), 4);
}

#[test]
fn oversized_parent_cancels_graph_work_and_still_completes_callbacks() {
    let (mut engine, _) = fixture();
    for i in 0..256 {
        engine.add_node(Folder::new(format!("extra {i}")).into(), None);
    }
    engine.apply_edits().unwrap();
    let mut delivery = InputDelivery::new(input(), vec![path(ParamValue::Int(42))], engine.root, mode());
    let (_, mut ctx) = service(&mut engine, &mut delivery);
    assert!(ctx
        .edits
        .pending
        .iter()
        .all(|e| matches!(e.edit, Edit::EmitCustomEvent { .. })));
    apply(&mut engine, &mut ctx);
    settle(&mut engine, &mut delivery);
    assert_eq!(callbacks(&engine), 2);
}

#[test]
fn relative_cc_does_not_recreate_a_disappeared_target_as_absolute() {
    let (mut engine, _) = fixture();
    let mut write = path(ParamValue::Int(42));
    write.rotary = Some(ROTARY_TWOS_COMPLEMENT);
    let mut delivery = InputDelivery::new(input(), vec![write], engine.root, mode());
    settle(&mut engine, &mut delivery);
    assert!(engine
        .process_tree_snapshot()
        .find_child_by_decl_id(engine.root, "channel_1")
        .is_none());
    assert_eq!(callbacks(&engine), 2);
}

#[test]
fn large_sysex_value_gets_its_own_turn_before_the_following_write() {
    let (mut engine, pulse) = fixture();
    engine.add_node(
        Parameter::new("bytes", ParamValue::Str(String::new()), ParameterChangeCheck::None).into(),
        None,
    );
    engine.apply_edits().unwrap();
    let bytes = engine.nodes.get(engine.root).unwrap().node_data().last_child.unwrap();
    let input = midi_input::InputDraft::prepared(MidiMessage::System(MidiSystemMessage::Sysex {
        bytes: vec![127; 65534],
    }));
    let mut write = Write::direct(bytes, ParamValue::Str(String::new()));
    write.value = Value::Hex;
    let mut delivery = InputDelivery::new(
        input,
        vec![write, Write::direct(pulse, ParamValue::Trigger())],
        engine.root,
        mode(),
    );
    let (_, mut ctx) = service(&mut engine, &mut delivery);
    assert_eq!(ctx.edits.pending.len(), 1);
    assert!(matches!(&ctx.edits.pending[0].edit, Edit::SetParam { node, .. } if *node == bytes));
    apply(&mut engine, &mut ctx);
    settle(&mut engine, &mut delivery);
    assert_eq!(pulses(&engine, pulse), 1);
    assert_eq!(callbacks(&engine), 2);
}

#[test]
fn module_disable_cancels_input_graph_callback_and_held_note_state() {
    let mut module = MidiModule::create();
    module.pending_incoming_messages.push_back(input());
    module.incoming = Some(InputDelivery::new(input(), vec![], NodeId(1), mode()));
    module
        .incoming_callbacks
        .push_back(InputDelivery::new(input(), vec![], NodeId(1), mode()));
    module.active_notes.insert((1, 60));
    module.ignored_param_changes.insert(NodeId(2), 1);
    let (engine, _) = fixture();
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    module.on_effective_enabled_changed(&mut ctx, false);
    assert!(module.pending_incoming_messages.is_empty());
    assert!(module.incoming.is_none());
    assert!(module.incoming_callbacks.is_empty());
    assert!(module.active_notes.is_empty());
    assert!(module.ignored_param_changes.is_empty());
}

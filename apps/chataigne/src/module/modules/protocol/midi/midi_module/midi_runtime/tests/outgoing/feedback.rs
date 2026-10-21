use super::super::super::super::{
    create_cc_rotary_mode_parameter, create_folder, create_int_parameter, ROTARY_ABSOLUTE, ROTARY_TWOS_COMPLEMENT,
};
use super::*;
use golden_core::{
    edit::{Edit, NodeTree},
    node::{Node, NodeId},
    parameter::{ParamValue, ParameterConstraints, ParameterEventBehaviour, RangeConstraint},
    process_ctx::ExecutionPhase,
};

fn module(engine: &mut crate::app::AppEngine, id: NodeId) -> &mut crate::app::MidiModule {
    engine.nodes.get_mut(id).unwrap().as_any_mut().downcast_mut().unwrap()
}
fn settle(engine: &mut crate::app::AppEngine) {
    for _ in 0..4 {
        engine.apply_edits().unwrap();
        engine.dispatch_inbox(ExecutionPhase::EngineTick).unwrap();
    }
    engine.apply_edits().unwrap();
}
fn value_edit(node: NodeId, value: ParamValue) -> Edit {
    Edit::SetParam {
        node,
        value,
        behaviour: ParameterEventBehaviour::Append,
    }
}
type SentPackets = Arc<Mutex<Vec<Vec<u8>>>>;

fn fixture(cc: bool, relative: bool) -> (crate::app::AppEngine, NodeId, NodeId, SentPackets) {
    let native = pool();
    let (mut engine, id, sent) = module_fixture(&native);
    let m = module(&mut engine, id);
    let values = m.base.values_id().unwrap();
    let auto = m.auto_feedback.id();
    let channel = create_folder("Channel 1", "channel_1");
    let mut leaf = NodeTree::new(create_int_parameter(
        "value",
        if cc { "cc_7" } else { "note_60" },
        if relative { 10 } else { 40 },
        Some((0, 127)),
        false,
    ));
    if cc {
        leaf.push_child(NodeTree::new(create_cc_rotary_mode_parameter(if relative {
            ROTARY_TWOS_COMPLEMENT
        } else {
            ROTARY_ABSOLUTE
        })));
    }
    engine.edits.push(Edit::AddNodeTree {
        parent: values,
        prev_sibling: None,
        tree: NodeTree::new(channel).with_child(
            NodeTree::new(create_folder("kind", if cc { "control_change" } else { "notes" })).with_child(leaf),
        ),
    });
    engine.edits.push(value_edit(auto, ParamValue::Bool(true)));
    settle(&mut engine);
    let snapshot = engine.process_tree_snapshot();
    let channel = snapshot.find_child_by_decl_id(values, "channel_1").unwrap();
    let folder = snapshot
        .find_child_by_decl_id(channel, if cc { "control_change" } else { "notes" })
        .unwrap();
    let value = snapshot
        .find_child_by_decl_id(folder, if cc { "cc_7" } else { "note_60" })
        .unwrap();
    (engine, id, value, sent)
}
fn finish(engine: &mut crate::app::AppEngine, id: NodeId) {
    let mut output = module(engine, id).output.take().unwrap();
    output.finish_admission();
    let mut events = Vec::new();
    until(|| output.drain(&mut events).state == PendingDrainState::Disconnected);
}

#[test]
fn unchanged_received_write_does_not_swallow_the_next_user_edit() {
    let (mut engine, id, note, sent) = fixture(false, false);
    engine.edits.push_from(id, value_edit(note, ParamValue::Int(40)));
    engine.edits.push(value_edit(note, ParamValue::Int(41)));
    settle(&mut engine);
    finish(&mut engine, id);
    assert_eq!(&*sent.lock().unwrap(), &[vec![0x90, 60, 41]]);
}

#[test]
fn mixed_received_and_authored_changes_keep_their_own_occurrences_and_values() {
    let (mut engine, id, note, sent) = fixture(false, false);
    engine.edits.push(value_edit(note, ParamValue::Int(41)));
    engine.edits.push_from(id, value_edit(note, ParamValue::Int(42)));
    engine.edits.push(value_edit(note, ParamValue::Int(43)));
    settle(&mut engine);
    finish(&mut engine, id);
    assert_eq!(&*sent.lock().unwrap(), &[vec![0x90, 60, 41], vec![0x90, 60, 43]]);
}

#[test]
fn rejected_received_suffix_cannot_suppress_a_later_authored_change() {
    let (mut engine, id, note, sent) = fixture(false, false);
    engine
        .edits
        .push_from(id, value_edit(NodeId(u64::MAX), ParamValue::Int(1)));
    engine.edits.push_from(id, value_edit(note, ParamValue::Int(42)));
    assert!(engine.apply_edits().is_err());
    engine.edits.push(value_edit(note, ParamValue::Int(41)));
    settle(&mut engine);
    finish(&mut engine, id);
    assert_eq!(&*sent.lock().unwrap(), &[vec![0x90, 60, 41]]);
}

#[test]
fn relative_feedback_uses_each_captured_delta_instead_of_the_final_snapshot() {
    let (mut engine, id, cc, sent) = fixture(true, true);
    engine.edits.push(value_edit(cc, ParamValue::Int(11)));
    engine.edits.push(value_edit(cc, ParamValue::Int(13)));
    settle(&mut engine);
    finish(&mut engine, id);
    assert_eq!(&*sent.lock().unwrap(), &[vec![0xB0, 7, 1], vec![0xB0, 7, 2]]);
}

#[test]
fn fourteen_bit_feedback_keeps_both_complete_pairs_in_authored_order() {
    let (mut engine, id, cc, sent) = fixture(true, false);
    let toggle = module(&mut engine, id).channel_1.id();
    engine.edits.push(value_edit(toggle, ParamValue::Bool(true)));
    settle(&mut engine);
    engine.edits.push(value_edit(cc, ParamValue::Int(0x1234)));
    engine.edits.push(value_edit(cc, ParamValue::Int(0x2345)));
    settle(&mut engine);
    finish(&mut engine, id);
    assert_eq!(
        &*sent.lock().unwrap(),
        &[
            vec![0xB0, 7, 0x24],
            vec![0xB0, 39, 0x34],
            vec![0xB0, 7, 0x46],
            vec![0xB0, 39, 0x45]
        ]
    );
}

#[test]
fn repeated_clock_triggers_are_feedback_and_received_clock_is_not_echoed() {
    let (mut engine, id, _, sent) = fixture(false, false);
    let start = module(&mut engine, id).start.id();
    engine.edits.push(value_edit(start, ParamValue::Trigger()));
    engine.edits.push_from(id, value_edit(start, ParamValue::Trigger()));
    engine.edits.push(value_edit(start, ParamValue::Trigger()));
    settle(&mut engine);
    finish(&mut engine, id);
    assert_eq!(&*sent.lock().unwrap(), &[vec![0xFA], vec![0xFA]]);
}

#[test]
fn another_runtime_producer_is_not_mistaken_for_received_midi() {
    let (mut engine, id, note, sent) = fixture(false, false);
    engine
        .edits
        .push_from(NodeId(u64::MAX), value_edit(note, ParamValue::Int(41)));
    settle(&mut engine);
    finish(&mut engine, id);
    assert_eq!(&*sent.lock().unwrap(), &[vec![0x90, 60, 41]]);
}

#[test]
fn internal_constraint_clamping_does_not_echo_or_swallow_a_later_user_value() {
    let (mut engine, id, note, sent) = fixture(false, false);
    engine.edits.push_from(
        id,
        Edit::SetParamConstraints {
            node: note,
            constraints: ParameterConstraints {
                range: Some(RangeConstraint::Uniform {
                    min: Some(0.0),
                    max: Some(20.0),
                }),
                ..Default::default()
            },
        },
    );
    engine.edits.push(value_edit(note, ParamValue::Int(19)));
    settle(&mut engine);
    finish(&mut engine, id);
    assert_eq!(&*sent.lock().unwrap(), &[vec![0x90, 60, 19]]);
}

#[test]
fn narrowing_cc_width_does_not_echo_internal_clamping_or_conversion() {
    let (mut engine, id, cc, sent) = fixture(true, false);
    let toggle = module(&mut engine, id).channel_1.id();
    engine.edits.push(value_edit(toggle, ParamValue::Bool(true)));
    settle(&mut engine);
    engine.edits.push_from(id, value_edit(cc, ParamValue::Int(0x1234)));
    settle(&mut engine);
    engine.edits.push(value_edit(toggle, ParamValue::Bool(false)));
    settle(&mut engine);
    engine.edits.push(value_edit(cc, ParamValue::Int(37)));
    settle(&mut engine);
    finish(&mut engine, id);
    assert_eq!(&*sent.lock().unwrap(), &[vec![0xB0, 7, 37]]);
}

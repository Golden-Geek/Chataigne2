use std::time::Duration;

use golden_core::{
    edit::Edit,
    node::{Folder, Node, NodeId, NodeUserPermissions},
    parameter::{ParamValue, ParameterEventBehaviour, RangeConstraint},
    process_ctx::ExecutionPhase,
};

use super::{
    MidiMessage, MidiModule,
    midi_message::{cc_decl_id, note_decl_id},
};

#[test]
fn midi_module_command_tester_advertises_all_midi_commands() {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    engine.add_node(MidiModule::create().into(), None);
    engine.apply_edits().expect("midi module should attach");
    for _ in 0..4 {
        engine.apply_edits().expect("midi defaults should materialize");
    }

    let module_id = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("module should be attached under root");
    let command_tester_id = find_path(&engine, module_id, "command_tester").expect("command tester should exist");

    let available_types = engine
        .nodes
        .get(command_tester_id)
        .expect("command tester should exist")
        .user_creatable_items()
        .into_iter()
        .map(|item| item.node_type)
        .collect::<Vec<_>>();

    assert_eq!(
        available_types.len(),
        MidiModule::module_command_types().len(),
        "midi command tester should advertise the full MIDI command catalog"
    );

    for command_type in MidiModule::module_command_types() {
        assert!(
            available_types.iter().any(|item_type| item_type == command_type),
            "missing MIDI command type {command_type}; available types were {available_types:?}"
        );
    }
}

#[test]
fn incoming_note_messages_create_one_direct_velocity_param() {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    engine.add_node(MidiModule::create().into(), None);
    engine.apply_edits().expect("midi module should attach");
    engine.resolve().expect("runtime schedule should resolve");

    let module_id = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("module should be attached under root");

    let crate::app::AppNode::MidiModule(module) = engine.nodes.get_mut(module_id).expect("module should exist") else {
        panic!("expected MidiModule node");
    };
    module.enqueue_incoming_message_for_test(MidiMessage::NoteOn {
        channel: 1,
        note: 24,
        velocity: 96,
    });

    for _ in 0..3 {
        run_midi_runtime_tick(&mut engine, "runtime tick should process queued MIDI note-on");
    }

    let crate::app::AppNode::MidiModule(module) = engine.nodes.get(module_id).expect("module should still exist")
    else {
        panic!("expected MidiModule node");
    };
    assert!(
        module.auto_add_enabled_for_test(),
        "auto-add should remain enabled by default"
    );

    let values_id = find_path(&engine, module_id, "values").expect("module should contain a Values folder");
    let channel_id = find_path(&engine, module_id, "values/channel_1").expect("channel folder should be created");
    let notes_id = find_path(&engine, module_id, "values/channel_1/notes").expect("notes folder should be created");
    let note_path = format!("values/channel_1/notes/{}", note_decl_id(24));
    let note_id = find_path(&engine, module_id, note_path.as_str())
        .expect("note parameter should be created directly under Notes");
    let values_children = child_descriptions(&engine, values_id);
    let channel_children = child_descriptions(&engine, channel_id);
    let notes_children = child_descriptions(&engine, notes_id);

    assert_eq!(
        child_count(&engine, values_id),
        1,
        "values should only contain one channel folder after one note; children were {values_children:?}"
    );
    assert_eq!(
        child_count(&engine, channel_id),
        1,
        "channel folder should only contain the Notes folder after one note; children were {channel_children:?}"
    );
    assert_eq!(
        child_count(&engine, notes_id),
        1,
        "notes folder should only contain one direct note parameter; children were {notes_children:?}"
    );

    let note_node = engine.nodes.get(note_id).expect("note parameter should exist");
    assert!(
        note_node.node_data().first_child.is_none(),
        "note should be a direct int parameter, not a wrapper node with children"
    );
    assert_eq!(
        note_node.engine_param_snapshot().map(|snapshot| snapshot.value),
        Some(ParamValue::Int(96))
    );

    let crate::app::AppNode::MidiModule(module) = engine.nodes.get_mut(module_id).expect("module should still exist")
    else {
        panic!("expected MidiModule node");
    };
    module.enqueue_incoming_message_for_test(MidiMessage::NoteOff {
        channel: 1,
        note: 24,
        velocity: 64,
    });

    for _ in 0..2 {
        run_midi_runtime_tick(&mut engine, "runtime tick should process queued MIDI note-off");
    }

    let note_node = engine
        .nodes
        .get(find_path(&engine, module_id, note_path.as_str()).expect("note parameter should remain after note-off"))
        .expect("note parameter should still exist");
    assert_eq!(
        note_node.engine_param_snapshot().map(|snapshot| snapshot.value),
        Some(ParamValue::Int(0)),
        "note-off should leave the same note parameter at zero"
    );
    assert_eq!(
        child_count(&engine, notes_id),
        1,
        "note-off should not duplicate note parameters"
    );
}

#[test]
fn channel_14_bit_toggle_merges_removes_and_restores_paired_ccs() {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    engine.add_node(MidiModule::create().into(), None);
    engine.apply_edits().expect("midi module should attach");
    engine.resolve().expect("runtime schedule should resolve");

    let module_id = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("module should be attached under root");

    let crate::app::AppNode::MidiModule(module) = engine.nodes.get_mut(module_id).expect("module should exist") else {
        panic!("expected MidiModule node");
    };
    module.enqueue_incoming_message_for_test(MidiMessage::ControlChange {
        channel: 1,
        controller: 7,
        value: 0x46,
    });
    module.enqueue_incoming_message_for_test(MidiMessage::ControlChange {
        channel: 1,
        controller: 39,
        value: 0x45,
    });

    for _ in 0..6 {
        run_midi_runtime_tick(&mut engine, "runtime tick should materialize paired CC parameters");
    }

    let cc_7_path = "values/channel_1/control_change/cc_7";
    let cc_39_path = "values/channel_1/control_change/cc_39";
    let toggle_id = find_path(&engine, module_id, "parameters/fourteen_bit_channels/channel_1")
        .expect("channel 1 14-bit toggle should exist");

    assert_eq!(
        engine
            .nodes
            .get(find_path(&engine, module_id, cc_7_path).expect("CC 7 should exist before merge"))
            .and_then(|node| node.engine_param_snapshot())
            .map(|snapshot| snapshot.value),
        Some(ParamValue::Int(0x46))
    );
    assert_eq!(
        engine
            .nodes
            .get(find_path(&engine, module_id, cc_39_path).expect("CC 39 should exist before merge"))
            .and_then(|node| node.engine_param_snapshot())
            .map(|snapshot| snapshot.value),
        Some(ParamValue::Int(0x45))
    );

    set_param(&mut engine, toggle_id, ParamValue::Bool(true));
    settle_midi_module_edits(&mut engine);

    let merged_cc_id = find_path(&engine, module_id, cc_7_path).expect("CC 7 should remain after enabling 14-bit");
    assert_eq!(
        engine
            .nodes
            .get(merged_cc_id)
            .and_then(|node| node.engine_param_snapshot())
            .map(|snapshot| snapshot.value),
        Some(ParamValue::Int(0x2345)),
        "CC 7 should become the merged 14-bit value"
    );
    assert!(
        find_path(&engine, module_id, cc_39_path).is_none(),
        "the paired LSB CC should be removed when the channel switches to 14-bit"
    );

    set_param(&mut engine, toggle_id, ParamValue::Bool(false));
    settle_midi_module_edits(&mut engine);

    assert_eq!(
        engine
            .nodes
            .get(find_path(&engine, module_id, cc_7_path).expect("CC 7 should remain after returning to 7-bit"))
            .and_then(|node| node.engine_param_snapshot())
            .map(|snapshot| snapshot.value),
        Some(ParamValue::Int(0x46)),
        "CC 7 should be restored to its 7-bit MSB value"
    );
    assert_eq!(
        engine
            .nodes
            .get(find_path(&engine, module_id, cc_39_path).expect("CC 39 should be recreated after returning to 7-bit"))
            .and_then(|node| node.engine_param_snapshot())
            .map(|snapshot| snapshot.value),
        Some(ParamValue::Int(0x45)),
        "CC 39 should be recreated from the stored 14-bit LSB value"
    );
}

#[test]
fn incoming_note_messages_keep_notes_sorted_by_pitch() {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    engine.add_node(MidiModule::create().into(), None);
    engine.apply_edits().expect("midi module should attach");
    engine.resolve().expect("runtime schedule should resolve");

    let module_id = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("module should be attached under root");

    let crate::app::AppNode::MidiModule(module) = engine.nodes.get_mut(module_id).expect("module should exist") else {
        panic!("expected MidiModule node");
    };
    module.enqueue_incoming_message_for_test(MidiMessage::NoteOn {
        channel: 1,
        note: 100,
        velocity: 90,
    });
    module.enqueue_incoming_message_for_test(MidiMessage::NoteOn {
        channel: 1,
        note: 24,
        velocity: 91,
    });
    module.enqueue_incoming_message_for_test(MidiMessage::NoteOn {
        channel: 1,
        note: 64,
        velocity: 92,
    });

    for _ in 0..8 {
        run_midi_runtime_tick(&mut engine, "runtime tick should keep notes ordered by pitch");
    }

    let notes_id = find_path(&engine, module_id, "values/channel_1/notes").expect("notes folder should exist");
    assert_eq!(
        child_decl_ids(&engine, notes_id),
        vec![note_decl_id(24), note_decl_id(64), note_decl_id(100)],
        "notes should be ordered by MIDI pitch, not insertion order"
    );
}

#[test]
fn incoming_control_changes_keep_ccs_sorted_by_number() {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    engine.add_node(MidiModule::create().into(), None);
    engine.apply_edits().expect("midi module should attach");
    engine.resolve().expect("runtime schedule should resolve");

    let module_id = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("module should be attached under root");

    let crate::app::AppNode::MidiModule(module) = engine.nodes.get_mut(module_id).expect("module should exist") else {
        panic!("expected MidiModule node");
    };
    module.enqueue_incoming_message_for_test(MidiMessage::ControlChange {
        channel: 1,
        controller: 39,
        value: 10,
    });
    module.enqueue_incoming_message_for_test(MidiMessage::ControlChange {
        channel: 1,
        controller: 100,
        value: 11,
    });
    module.enqueue_incoming_message_for_test(MidiMessage::ControlChange {
        channel: 1,
        controller: 7,
        value: 12,
    });

    for _ in 0..8 {
        run_midi_runtime_tick(&mut engine, "runtime tick should keep CCs ordered by controller number");
    }

    let cc_id =
        find_path(&engine, module_id, "values/channel_1/control_change").expect("control change folder should exist");
    assert_eq!(
        child_decl_ids(&engine, cc_id),
        vec![cc_decl_id(7), cc_decl_id(39), cc_decl_id(100)],
        "CCs should be ordered by controller number, not insertion order"
    );
}

#[test]
fn midi_value_nodes_lock_permissions_and_cc_range_tracks_14_bit_mode() {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    engine.add_node(MidiModule::create().into(), None);
    engine.apply_edits().expect("midi module should attach");
    engine.resolve().expect("runtime schedule should resolve");

    let module_id = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("module should be attached under root");

    let crate::app::AppNode::MidiModule(module) = engine.nodes.get_mut(module_id).expect("module should exist") else {
        panic!("expected MidiModule node");
    };
    module.enqueue_incoming_message_for_test(MidiMessage::NoteOn {
        channel: 1,
        note: 24,
        velocity: 96,
    });
    module.enqueue_incoming_message_for_test(MidiMessage::ControlChange {
        channel: 1,
        controller: 7,
        value: 70,
    });

    for _ in 0..6 {
        run_midi_runtime_tick(&mut engine, "runtime tick should materialize MIDI value nodes with fixed metadata");
    }

    let values_id = find_path(&engine, module_id, "values").expect("values folder should exist");
    let note_id = find_path(&engine, module_id, "values/channel_1/notes/note_24")
        .expect("note parameter should exist");
    let cc_id =
        find_path(&engine, module_id, "values/channel_1/control_change/cc_7").expect("CC parameter should exist");
    let toggle_id = find_path(&engine, module_id, "parameters/fourteen_bit_channels/channel_1")
        .expect("channel 1 14-bit toggle should exist");

    assert_node_permissions_none(&engine, values_id);
    assert_node_permissions_none(&engine, note_id);
    assert_node_permissions_none(&engine, cc_id);
    assert_uniform_int_range(&engine, note_id, 0.0, 127.0);
    assert_uniform_int_range(&engine, cc_id, 0.0, 127.0);

    set_param(&mut engine, toggle_id, ParamValue::Bool(true));
    settle_midi_module_edits(&mut engine);

    let cc_id =
        find_path(&engine, module_id, "values/channel_1/control_change/cc_7").expect("merged CC should exist");
    assert_node_permissions_none(&engine, cc_id);
    assert_uniform_int_range(&engine, cc_id, 0.0, 16383.0);

    set_param(&mut engine, toggle_id, ParamValue::Bool(false));
    settle_midi_module_edits(&mut engine);

    let cc_id = find_path(&engine, module_id, "values/channel_1/control_change/cc_7")
        .expect("restored 7-bit CC should exist");
    assert_uniform_int_range(&engine, cc_id, 0.0, 127.0);
}

fn find_child_by_key(engine: &crate::app::AppEngine, parent: NodeId, key: &str) -> Option<NodeId> {
    let mut child = engine.nodes.get(parent)?.node_data().first_child;
    while let Some(child_id) = child {
        let node = engine.nodes.get(child_id)?;
        let meta = &node.node_data().meta;
        if meta.decl_id.0 == key
            || meta.decl_id.0.rsplit('/').next() == Some(key)
            || meta.short_name == key
            || meta.label == key
        {
            return Some(child_id);
        }
        child = node.node_data().next_sibling;
    }

    None
}

fn find_path(engine: &crate::app::AppEngine, start: NodeId, path: &str) -> Option<NodeId> {
    let mut current = start;
    let mut remaining = path.trim_matches('/');

    loop {
        if remaining.is_empty() {
            return Some(current);
        }

        if let Some(found) = find_child_by_key(engine, current, remaining) {
            return Some(found);
        }

        let Some((segment, tail)) = remaining.split_once('/') else {
            return find_child_by_key(engine, current, remaining);
        };
        current = find_child_by_key(engine, current, segment)?;
        remaining = tail;
    }
}

fn child_count(engine: &crate::app::AppEngine, parent: NodeId) -> usize {
    let mut count = 0;
    let mut child = engine.nodes.get(parent).and_then(|node| node.node_data().first_child);
    while let Some(child_id) = child {
        count += 1;
        child = engine
            .nodes
            .get(child_id)
            .and_then(|node| node.node_data().next_sibling);
    }
    count
}

fn child_descriptions(engine: &crate::app::AppEngine, parent: NodeId) -> Vec<String> {
    let mut labels = Vec::new();
    let mut child = engine.nodes.get(parent).and_then(|node| node.node_data().first_child);
    while let Some(child_id) = child {
        let node = engine.nodes.get(child_id).expect("child node should exist");
        labels.push(format!(
            "{} [{}]",
            node.node_data().meta.label,
            node.node_data().meta.decl_id.0
        ));
        child = node.node_data().next_sibling;
    }
    labels
}

fn child_decl_ids(engine: &crate::app::AppEngine, parent: NodeId) -> Vec<String> {
    let mut decl_ids = Vec::new();
    let mut child = engine.nodes.get(parent).and_then(|node| node.node_data().first_child);
    while let Some(child_id) = child {
        let node = engine.nodes.get(child_id).expect("child node should exist");
        decl_ids.push(node.node_data().meta.decl_id.0.clone());
        child = node.node_data().next_sibling;
    }
    decl_ids
}

fn run_midi_runtime_tick(engine: &mut crate::app::AppEngine, context: &str) {
    engine
        .dispatch_inbox(ExecutionPhase::EngineTick)
        .expect("pending MIDI events should dispatch");
    engine.apply_edits().expect("pending MIDI event reactions should apply");
    engine.run_tick(Duration::from_millis(20)).expect(context);
    engine.apply_edits().expect("pending MIDI edits should apply");
    engine
        .resolve()
        .expect("MIDI runtime schedule should resolve after edits");
}

fn set_param(engine: &mut crate::app::AppEngine, node: NodeId, value: ParamValue) {
    engine.edits.push(Edit::SetParam {
        node,
        value,
        behaviour: ParameterEventBehaviour::Coalesce,
    });
}

fn settle_midi_module_edits(engine: &mut crate::app::AppEngine) {
    engine.apply_edits().expect("pending MIDI edits should apply");
    engine
        .dispatch_inbox(ExecutionPhase::EngineTick)
        .expect("pending MIDI param change events should dispatch");
    engine.apply_edits().expect("follow-up MIDI edits should apply");
    engine
        .apply_edits()
        .expect("deferred MIDI conversion edits should apply");
    engine.resolve().expect("MIDI schedule should resolve after edits");
}

fn assert_node_permissions_none(engine: &crate::app::AppEngine, node: NodeId) {
    let actual = engine
        .nodes
        .get(node)
        .expect("node should exist")
        .node_data()
        .meta
        .user_permissions
        .clone();
    assert_eq!(actual, NodeUserPermissions::none(), "MIDI value node should not be user-configurable");
}

fn assert_uniform_int_range(engine: &crate::app::AppEngine, node: NodeId, min: f64, max: f64) {
    let snapshot = engine
        .nodes
        .get(node)
        .expect("parameter should exist")
        .engine_param_snapshot()
        .expect("node should be an int parameter");
    assert_eq!(
        snapshot.constraints.range,
        Some(RangeConstraint::Uniform {
            min: Some(min),
            max: Some(max),
        }),
        "parameter should expose the expected fixed MIDI range"
    );
}

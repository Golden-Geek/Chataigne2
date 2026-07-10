use golden_core::{
    edit::Edit,
    node::{Folder, Node, NodeId},
    parameter::{ParamValue, ParameterEventBehaviour},
    process_ctx::ExecutionPhase,
};

use super::{
    validate_14_bit_controller, MidiSendNoteOnCommand, NOTE_MODE_OCTAVE_NOTE, NOTE_MODE_PITCH,
};

#[test]
fn validate_14_bit_controller_accepts_msb_range() {
    assert_eq!(validate_14_bit_controller(0), Ok(0));
    assert_eq!(validate_14_bit_controller(31), Ok(31));
}

#[test]
fn validate_14_bit_controller_rejects_lsb_range() {
    let error = validate_14_bit_controller(32).expect_err("controller 32 should be reserved for the paired LSB role");
    assert!(error.contains("0-31 range"));
    assert!(error.contains("32"));
}

#[test]
fn note_command_dependencies_follow_note_mode() {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    engine.add_node(MidiSendNoteOnCommand::create().into(), None);
    stabilize_dependencies(&mut engine, "note-on command should attach");

    let command_id = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("note-on command should be attached under root");

    assert_eq!(
        direct_child_decl_ids(&engine, command_id),
        ["target_module", "trigger", "channel", "note_mode", "pitch", "velocity"]
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>(),
        "pitch mode should only materialize the pitch selector"
    );

    let note_mode = find_direct_child_by_decl(&engine, command_id, "note_mode")
        .expect("note mode parameter should exist");
    set_param(&mut engine, note_mode, ParamValue::Enum(NOTE_MODE_OCTAVE_NOTE.to_string()));
    stabilize_dependencies(&mut engine, "note mode change should update dependent note params");

    assert_eq!(
        direct_child_decl_ids(&engine, command_id),
        ["target_module", "trigger", "channel", "note_mode", "octave", "note", "velocity"]
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>(),
        "octave-note mode should replace pitch with octave and note"
    );

    set_param(&mut engine, note_mode, ParamValue::Enum(NOTE_MODE_PITCH.to_string()));
    stabilize_dependencies(&mut engine, "switching back to pitch should restore the pitch selector");

    assert_eq!(
        direct_child_decl_ids(&engine, command_id),
        ["target_module", "trigger", "channel", "note_mode", "pitch", "velocity"]
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>(),
        "switching back to pitch should remove octave-note selectors"
    );
}

fn find_direct_child_by_decl(engine: &crate::app::AppEngine, parent: NodeId, decl_id: &str) -> Option<NodeId> {
    let mut child = engine.nodes.get(parent).and_then(|node| node.node_data().first_child);
    while let Some(child_id) = child {
        let child_node = engine.nodes.get(child_id)?;
        if child_node.node_data().meta.decl_id.0 == decl_id {
            return Some(child_id);
        }
        child = child_node.node_data().next_sibling;
    }
    None
}

fn direct_child_decl_ids(engine: &crate::app::AppEngine, parent: NodeId) -> Vec<String> {
    let mut decl_ids = Vec::new();
    let mut child = engine.nodes.get(parent).and_then(|node| node.node_data().first_child);
    while let Some(child_id) = child {
        let child_node = engine.nodes.get(child_id).expect("child should exist");
        decl_ids.push(child_node.node_data().meta.decl_id.0.clone());
        child = child_node.node_data().next_sibling;
    }
    decl_ids
}

fn set_param(engine: &mut crate::app::AppEngine, node: NodeId, value: ParamValue) {
    engine.edits.push(Edit::SetParam {
        node,
        value,
        behaviour: ParameterEventBehaviour::Coalesce,
    });
}

fn stabilize_dependencies(engine: &mut crate::app::AppEngine, reason: &str) {
    for _ in 0..6 {
        engine.apply_edits().expect(reason);
        engine
            .dispatch_inbox(ExecutionPhase::EndOfTickStabilization)
            .expect("dependency stabilization dispatch should succeed");
    }
}

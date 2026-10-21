use super::*;
use golden_core::{
    events::CustomEvent,
    node::{Folder, NodeUuid},
    process_ctx::ProcessTreeNodeSnapshot,
};
use std::{collections::HashMap, sync::Arc};

fn invocation(kind: &str, values: Vec<(&str, ParamValue)>) -> CapturedInvocation {
    let engine = crate::app::AppEngine::new(Folder::new("template").into());
    let template = engine.process_tree_snapshot().node(engine.root).unwrap().clone();
    let mut nodes = HashMap::new();
    let mut root = template.clone();
    root.id = NodeId(1);
    root.node_type = kind.into();
    root.parent = None;
    root.first_child = (!values.is_empty()).then_some(NodeId(2));
    root.child_count = values.len();
    root.enabled = true;
    nodes.insert(root.id, root);
    let count = values.len();
    for (index, (decl, value)) in values.into_iter().enumerate() {
        let id = NodeId(index as u64 + 2);
        let mut node: ProcessTreeNodeSnapshot = template.clone();
        node.id = id;
        node.uuid = NodeUuid(uuid::Uuid::from_u128(id.0 as u128));
        node.parent = Some(NodeId(1));
        node.first_child = None;
        node.child_count = 0;
        node.next_sibling = (index + 1 < count).then_some(NodeId(id.0 + 1));
        node.decl_id = decl.into();
        node.node_type = "parameter".into();
        node.param_value = Some(value);
        node.enabled = true;
        nodes.insert(id, node);
    }
    CapturedInvocation {
        snapshot: Arc::new(ProcessTreeSnapshot::new(NodeId(1), nodes))
            .try_lease()
            .unwrap(),
        execution: None,
        module: NodeId(1),
        command: NodeId(1),
        override_weight: 0,
    }
}
#[test]
fn all_thirteen_command_builders_preserve_their_typed_default_packets() {
    let cases = [
        ("midi_send_note_on_command", vec![vec![0x90, 60, 127]]),
        ("midi_send_note_off_command", vec![vec![0x80, 60, 0]]),
        (
            "midi_send_full_note_command",
            vec![vec![0x90, 60, 127], vec![0x80, 60, 0]],
        ),
        ("midi_send_control_change_command", vec![vec![0xB0, 1, 0]]),
        ("midi_send_program_change_command", vec![vec![0xC0, 0]]),
        ("midi_send_pitch_bend_command", vec![vec![0xE0, 0, 0x40]]),
        ("midi_send_channel_pressure_command", vec![vec![0xD0, 0]]),
        ("midi_send_poly_pressure_command", vec![vec![0xA0, 60, 0]]),
        ("midi_send_system_common_command", vec![vec![0xF6]]),
        ("midi_send_system_realtime_command", vec![vec![0xFA]]),
        ("midi_send_sysex_bytes_command", vec![vec![0xF0, 0xF7]]),
        ("midi_send_sysex_string_command", vec![vec![0xF0, 0xF7]]),
        ("midi_send_raw_bytes_command", vec![vec![]]),
    ];
    for (kind, expected) in cases {
        let request = prepare_midi_command(&invocation(kind, Vec::new())).unwrap();
        assert_eq!(
            request.packets.iter().map(|p| p.bytes.clone()).collect::<Vec<_>>(),
            expected,
            "{kind}"
        );
    }
}
#[test]
fn captured_overrides_use_the_last_value_without_mutating_the_frozen_snapshot() {
    use crate::app::module_command::{ModuleCommandDeliveryPolicy, ModuleCommandParamOverride};
    let mut input = invocation("midi_send_note_on_command", vec![("pitch", ParamValue::Int(60))]);
    let payload = ModuleCommandExecuteEvent {
        command_id: input.command,
        param_overrides: vec![
            ModuleCommandParamOverride {
                param_id: NodeId(2),
                value: ParamValue::Int(42),
            },
            ModuleCommandParamOverride {
                param_id: NodeId(2),
                value: ParamValue::Int(43),
            },
        ],
        invocation_id: None,
        delivery_policy: ModuleCommandDeliveryPolicy::Standard,
    };
    input.execution = Some(
        CustomEvent::new("execute", Some(input.command), serde_json::to_value(payload).unwrap())
            .try_lease_payload(4096)
            .unwrap(),
    );
    input.override_weight = 4096;
    assert_eq!(prepare_midi_command(&input).unwrap().packets[0].bytes, [0x90, 43, 127]);
    assert_eq!(
        input.snapshot.node(NodeId(2)).unwrap().param_value,
        Some(ParamValue::Int(60))
    );
}
#[test]
fn malformed_or_oversized_sources_fail_before_a_native_plan_is_sealed() {
    let extreme = invocation(
        "midi_send_note_on_command",
        vec![
            ("note_mode", ParamValue::Enum("octave_note".into())),
            ("octave", ParamValue::Int(i32::MAX)),
        ],
    );
    assert!(prepare_midi_command(&extreme).is_err());
    for value in ["FF ".repeat(65537), " ".repeat(256 * 1024 + 1), "not a byte".into()] {
        assert!(prepare_midi_command(&invocation(
            "midi_send_raw_bytes_command",
            vec![("bytes", ParamValue::Str(value))]
        ))
        .is_err());
    }
    assert!(prepare_midi_command(&invocation(
        "midi_send_system_realtime_command",
        vec![("message", ParamValue::Enum("x".repeat(129)))]
    ))
    .is_err());
}
#[test]
fn fourteen_bit_cc_prepares_the_complete_msb_lsb_pair_and_note_mode_is_backend_owned() {
    let input = invocation(
        "midi_send_control_change_command",
        vec![
            ("channel", ParamValue::Int(2)),
            ("controller", ParamValue::Int(1)),
            ("is_14_bit", ParamValue::Bool(true)),
            ("value_14_bit", ParamValue::Int(0x1234)),
        ],
    );
    assert_eq!(
        prepare_midi_command(&input)
            .unwrap()
            .packets
            .iter()
            .map(|p| p.bytes.clone())
            .collect::<Vec<_>>(),
        [vec![0xB1, 1, 0x24], vec![0xB1, 33, 0x34]]
    );
    let input = invocation(
        "midi_send_note_on_command",
        vec![
            ("note_mode", ParamValue::Enum("octave_note".into())),
            ("note", ParamValue::Enum("C".into())),
            ("octave", ParamValue::Int(4)),
        ],
    );
    assert_eq!(prepare_midi_command(&input).unwrap().packets[0].bytes, [0x90, 60, 127]);
}

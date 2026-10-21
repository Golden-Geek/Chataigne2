use super::*;
use golden_core::{
    edit::Edit,
    node::{Folder, NodeMetaPatch},
    parameter::ParameterChangeCheck,
    process_ctx::ExecutionPhase,
};

fn port(index: usize, label: &str) -> DiscoveredMidiPort {
    DiscoveredMidiPort {
        variant_id: midi_port_variant_id(index, label),
        label: label.into(),
        details: String::new(),
    }
}

#[test]
fn native_inventory_is_admitted_before_name_conversion() {
    let mut calls = 0;
    let result = describe_ports(vec![0u8; 65], |_| {
        calls += 1;
        Ok::<_, String>("fake".into())
    });
    assert!(result.is_err());
    assert_eq!(calls, 0);
    assert!(describe_ports(vec![0u8], |_| Ok::<_, String>("x".repeat(513))).is_err());
    let ports = describe_ports(vec![0u8], |_| Ok::<_, String>("x".repeat(512))).unwrap();
    assert_eq!(ports.len(), 1);
}

#[test]
fn availability_matches_connection_resolution_without_retargeting_reindexed_name() {
    let ports = || Ok(vec![port(0, "same")]);
    assert!(!midi_port_available("1|same", ports).unwrap());
    assert!(resolve_port(vec![0u8], "1|same", |_, _| Ok("same".into())).is_err());
    assert!(midi_port_available("0|same", ports).unwrap());
    assert!(resolve_port(vec![0u8], "0|same", |_, _| Ok("same".into()))
        .unwrap()
        .is_some());
    assert!(midi_port_available("same", ports).unwrap());
    assert!(resolve_port(vec![0u8], "same", |_, _| Ok("same".into()))
        .unwrap()
        .is_some());
}

fn fixture() -> (crate::app::AppEngine, NodeId) {
    let mut engine = crate::app::AppEngine::new(Folder::new("root").into());
    let mut parameter = Parameter::new(
        "Port",
        ParamValue::Enum("none".into()),
        ParameterChangeCheck::ValueChange,
    );
    parameter.constraints.enum_options = midi_input_port_options(&[port(0, "first")]).into();
    engine.add_node(parameter.into(), None);
    engine.apply_edits().unwrap();
    let id = engine.nodes.get(engine.root).unwrap().node_data().first_child.unwrap();
    (engine, id)
}

#[test]
fn a_queued_catalog_preserves_later_user_selection_and_metadata() {
    let (mut engine, id) = fixture();
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    sync_midi_port_enum_options(&mut ctx, id, midi_input_port_options(&[]));
    engine.absorb_edits(&mut ctx).unwrap();
    engine.edits.push(Edit::SetParam {
        node: id,
        value: ParamValue::Enum("0|first".into()),
        behaviour: Default::default(),
    });
    engine.edits.push(Edit::PatchMeta {
        node: id,
        patch: NodeMetaPatch {
            label: Some("Authored port".into()),
            ..Default::default()
        },
    });
    engine.apply_edits().unwrap();
    engine.apply_edits().unwrap();
    let node = engine.nodes.get(id).unwrap();
    assert_eq!(node.node_data().meta.label, "Authored port");
    assert_eq!(
        node.engine_param_snapshot().unwrap().value,
        ParamValue::Enum("0|first".into())
    );
}

#[test]
fn missing_port_catalog_preserves_selected_identity() {
    let (mut engine, id) = fixture();
    engine.edits.push(Edit::SetParam {
        node: id,
        value: ParamValue::Enum("0|first".into()),
        behaviour: Default::default(),
    });
    engine.apply_edits().unwrap();
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    sync_midi_port_enum_options(&mut ctx, id, midi_input_port_options(&[]));
    engine.absorb_edits(&mut ctx).unwrap();
    engine.apply_edits().unwrap();
    engine.apply_edits().unwrap();
    let snapshot = engine.nodes.get(id).unwrap().engine_param_snapshot().unwrap();
    assert_eq!(snapshot.value, ParamValue::Enum("0|first".into()));
    let missing = snapshot
        .constraints
        .enum_options
        .iter()
        .find(|o| o.variant_id == "0|first")
        .unwrap();
    assert!(missing.tags.iter().any(|tag| tag == "missing"));
}

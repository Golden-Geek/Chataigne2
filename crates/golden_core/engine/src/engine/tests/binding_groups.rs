use crate::{
    define_node_enum,
    edit::Edit,
    engine::Engine,
    node::{Folder, Node, NodeId},
    parameter::{ParamValue, Parameter, ParameterChangeCheck, ParameterEventBehaviour},
};

define_node_enum!(
    enum BindingTestNode {}
);

fn fixture(count: usize) -> (Engine<BindingTestNode>, Vec<NodeId>) {
    let mut engine = Engine::new(BindingTestNode::from(Folder::new("Root")));
    for index in 0..count {
        engine.add_node(
            Parameter::new(
                &format!("Value {index}"),
                ParamValue::Float(index as f64),
                ParameterChangeCheck::ValueChange,
            )
            .into(),
            None,
        );
    }
    engine.apply_edits().unwrap();
    let mut members = Vec::new();
    let mut next = engine.nodes.get(engine.root).unwrap().node_data().first_child;
    while let Some(id) = next {
        members.push(id);
        next = engine.nodes.get(id).unwrap().node_data().next_sibling;
    }
    (engine, members)
}

#[test]
fn a_leaf_write_reaches_a_large_group_in_one_evaluation_without_echoes() {
    let (mut engine, members) = fixture(80);
    engine.queue_parameter_binding_group(&members, members[0]).unwrap();
    engine.settle_edit_notifications(16).unwrap();
    for &author in &[members[79], members[1], members[0]] {
        engine.edits.push(Edit::SetParam {
            node: author,
            value: ParamValue::Float(author.0 as f64 + 100.0),
            behaviour: ParameterEventBehaviour::Append,
        });
        engine.apply_edits().unwrap();
        engine.evaluate_parameter_controls();
        engine.apply_edits().unwrap();
        let expected = engine.nodes.get(author).unwrap().engine_param_snapshot().unwrap().value;
        for &member in &members {
            assert_eq!(
                engine.nodes.get(member).unwrap().engine_param_snapshot().unwrap().value,
                expected
            );
        }
        engine.evaluate_parameter_controls();
        assert!(
            engine.edits.pending.is_empty(),
            "the completed component must not echo into later ticks"
        );
    }
}

#[test]
fn incomplete_or_duplicate_groups_reject_before_queuing_any_mutation() {
    let (mut engine, members) = fixture(4);
    engine.queue_parameter_binding_group(&members[..3], members[0]).unwrap();
    engine.settle_edit_notifications(16).unwrap();
    for proposal in [vec![members[0], members[3]], vec![members[0], members[0]]] {
        assert!(engine.queue_parameter_binding_group(&proposal, members[0]).is_err());
        assert!(engine.edits.pending.is_empty());
        assert_eq!(engine.parameter_binding_group(members[0]).len(), 3);
    }
}

#[test]
fn retargeting_a_reference_updates_component_reads_before_the_next_tick() {
    let (mut engine, members) = fixture(3);
    engine.queue_parameter_binding_group(&members[..2], members[0]).unwrap();
    engine.settle_edit_notifications(16).unwrap();
    let reference = engine.nodes.get(members[1]).unwrap().node_data().first_child.unwrap();
    let uuid = engine.nodes.get(members[2]).unwrap().node_data().meta.uuid;
    engine.edits.push(Edit::SetParam {
        node: reference,
        value: ParamValue::Reference(crate::node::NodeReference::new(uuid)),
        behaviour: ParameterEventBehaviour::Append,
    });
    engine.apply_edits().unwrap();
    assert_eq!(engine.parameter_binding_group(members[0]), vec![members[0]]);
    assert_eq!(engine.parameter_binding_group(members[2]), vec![members[1], members[2]]);
    assert!(engine.undo().unwrap());
    assert_eq!(engine.parameter_binding_group(members[0]), vec![members[0], members[1]]);
}

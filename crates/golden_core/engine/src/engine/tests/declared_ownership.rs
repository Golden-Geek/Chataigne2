use crate::{
    app::ProjectNode,
    edit::{Edit, NodeTree},
    engine::Engine,
    node::{Folder, Node, NodeId},
    parameter::{ParamValue, ParameterEventBehaviour},
};

#[crate::node("scoped_parameter_owner")]
#[children(folder(parameters) { value: f64 = 0.0; })]
struct ScopedOwner {}
#[crate::node("scoped_parameter_owner", from_struct)]
impl Node for ScopedOwner {}

crate::define_node_enum!(
    enum TestNode {
        Owner(ScopedOwner),
    }
);

fn value(engine: &Engine<TestNode>, owner: NodeId) -> NodeId {
    match engine.nodes.get(owner).unwrap() {
        TestNode::Owner(node) => node.value.id(),
        _ => unreachable!(),
    }
}

#[test]
fn identical_declaration_paths_in_nested_owners_never_materialize_or_bind_across_owners() {
    let mut engine = Engine::<TestNode>::new(Folder::new("root").into());
    engine.edits.push(Edit::AddNodeTree {
        parent: engine.root,
        prev_sibling: None,
        tree: NodeTree::new(ScopedOwner::new()).with_child(NodeTree::new(ScopedOwner::new())),
    });
    engine.apply_edits().unwrap();
    engine
        .dispatch_inbox(crate::process_ctx::ExecutionPhase::EndOfTickStabilization)
        .unwrap();
    let parent = engine.nodes.get(engine.root).unwrap().node_data().first_child.unwrap();
    let child = engine.nodes.get(parent).unwrap().node_data().first_child.unwrap();
    assert_eq!(
        engine.nodes.len(),
        7,
        "Each owner owns exactly its folder and parameter"
    );
    let parent_value = value(&engine, parent);
    let child_value = value(&engine, child);
    assert_ne!(parent_value, child_value);
    engine.edits.push(Edit::SetParam {
        node: child_value,
        value: ParamValue::Float(0.75),
        behaviour: ParameterEventBehaviour::Append,
    });
    engine.apply_edits().unwrap();
    engine
        .dispatch_inbox(crate::process_ctx::ExecutionPhase::EndOfTickStabilization)
        .unwrap();
    match engine.nodes.get(parent).unwrap() {
        TestNode::Owner(node) => assert_eq!(node.value.get(), 0.0),
        _ => unreachable!(),
    }
    match engine.nodes.get(child).unwrap() {
        TestNode::Owner(node) => assert_eq!(node.value.get(), 0.75),
        _ => unreachable!(),
    }
    let mut replacement = crate::parameter::Parameter::new("Value", ParamValue::Float(0.5), Default::default());
    replacement.node_data_mut().meta.decl_id = crate::node::DeclId("parameters/value".into());
    engine.edits.push(Edit::ReplaceNode {
        node: child_value,
        new_node: Box::new(replacement),
    });
    engine.apply_edits().unwrap();
    engine
        .dispatch_inbox(crate::process_ctx::ExecutionPhase::EndOfTickStabilization)
        .unwrap();
    assert_eq!(engine.nodes.len(), 7);
    assert_eq!(value(&engine, parent), parent_value);
    assert_ne!(value(&engine, child), parent_value);
    match engine.nodes.get(parent).unwrap() {
        TestNode::Owner(node) => assert_eq!(node.value.get(), 0.0),
        _ => unreachable!(),
    }
    engine.edits.push(Edit::ReplaceNode {
        node: child,
        new_node: Box::new(ScopedOwner::new()),
    });
    engine.apply_edits().unwrap();
    engine
        .dispatch_inbox(crate::process_ctx::ExecutionPhase::EndOfTickStabilization)
        .unwrap();
    // ReplaceNode retains the live slot and children; its payload has a new UUID.
    assert_eq!(engine.nodes.len(), 7);
    match engine.nodes.get(child).unwrap() {
        TestNode::Owner(node) => assert_eq!(node.value.get(), 0.5),
        _ => unreachable!(),
    }
    let parent_uuid = engine.nodes.get(parent).unwrap().node_data().meta.uuid;
    let child_uuid = engine.nodes.get(child).unwrap().node_data().meta.uuid;
    let project = engine.to_project_file_with(Node::project_encode_data).unwrap();
    let loaded = Engine::<TestNode>::from_project_file_with(project, TestNode::project_decode_node).unwrap();
    let parent = loaded.node_id_by_uuid(parent_uuid).unwrap();
    let child = loaded.node_id_by_uuid(child_uuid).unwrap();
    assert_eq!(loaded.nodes.len(), 7);
    match loaded.nodes.get(parent).unwrap() {
        TestNode::Owner(node) => assert_eq!(node.value.get(), 0.0),
        _ => unreachable!(),
    }
    match loaded.nodes.get(child).unwrap() {
        TestNode::Owner(node) => assert_eq!(node.value.get(), 0.5),
        _ => unreachable!(),
    }
}

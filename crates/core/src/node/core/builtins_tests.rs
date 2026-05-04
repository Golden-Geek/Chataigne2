use super::{Node, UserContextNode};

#[test]
fn user_context_node_shows_in_nested_inspector_by_default() {
    let node = UserContextNode::new("Context");

    assert!(node.node_data().meta.presentation.show_in_nested_inspector);
}

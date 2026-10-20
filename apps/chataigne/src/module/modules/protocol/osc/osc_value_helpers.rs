use golden_core::{node::NodeId, process_ctx::ProcessTreeSnapshot};

pub(super) fn is_descendant_of_node(
    snapshot: &ProcessTreeSnapshot,
    start: NodeId,
    ancestor: NodeId,
) -> Result<bool, &'static str> {
    let mut current = Some(start);
    for _ in 0..=64 {
        let Some(node) = current else { return Ok(false) };
        if node == ancestor {
            return Ok(true);
        }
        current = snapshot.node(node).and_then(|node| node.parent);
    }
    Err("OSC feedback path exceeds 64 levels; value was not admitted")
}

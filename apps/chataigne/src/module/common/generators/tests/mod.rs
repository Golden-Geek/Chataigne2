use super::*;
use golden_core::{
    node::{Folder, NodeId, NodeUuid},
    parameter::ParamValue,
    process_ctx::{ProcessTreeNodeSnapshot, ProcessTreeSnapshot},
};
use std::collections::HashMap;

struct SnapshotBuilder {
    template: ProcessTreeNodeSnapshot,
    nodes: HashMap<NodeId, ProcessTreeNodeSnapshot>,
    tails: HashMap<NodeId, NodeId>,
}

impl SnapshotBuilder {
    fn new() -> Self {
        let engine = crate::app::AppEngine::new(Folder::new("list").into());
        let template = engine.process_tree_snapshot().node(engine.root).unwrap().clone();
        let mut builder = Self {
            template,
            nodes: HashMap::new(),
            tails: HashMap::new(),
        };
        builder.add(None, "list", None);
        builder
    }

    fn add(&mut self, parent: Option<NodeId>, decl: &str, value: Option<ParamValue>) -> NodeId {
        let id = NodeId(self.nodes.len() as u64 + 1);
        let mut node = self.template.clone();
        node.id = id;
        node.uuid = NodeUuid(uuid::Uuid::from_u128(id.0 as u128));
        node.parent = parent;
        node.first_child = None;
        node.next_sibling = None;
        node.child_count = 0;
        node.decl_id = decl.into();
        node.label = decl.into();
        node.param_value = value;
        self.nodes.insert(id, node);
        if let Some(parent) = parent {
            if let Some(tail) = self.tails.insert(parent, id) {
                self.nodes.get_mut(&tail).unwrap().next_sibling = Some(id);
            } else {
                self.nodes.get_mut(&parent).unwrap().first_child = Some(id);
            }
            self.nodes.get_mut(&parent).unwrap().child_count += 1;
        }
        id
    }

    fn snapshot(self) -> ProcessTreeSnapshot {
        ProcessTreeSnapshot::new(NodeId(1), self.nodes)
    }
}

#[test]
fn preflight_rejects_item_and_label_bounds_in_borrowed_metadata() {
    let mut builder = SnapshotBuilder::new();
    for _ in 0..=MAX_ITEMS {
        builder.add(Some(NodeId(1)), "item", None);
    }
    assert!(preflight(&builder.snapshot(), NodeId(1), false).is_err());
    let mut builder = SnapshotBuilder::new();
    builder.add(Some(NodeId(1)), &"x".repeat(MAX_LABEL_BYTES + 1), None);
    assert!(preflight(&builder.snapshot(), NodeId(1), false).is_err());
}

#[test]
fn preflight_bounds_total_curve_structure_before_parsing_keys() {
    let mut builder = SnapshotBuilder::new();
    let item = builder.add(Some(NodeId(1)), "item", None);
    let curve = builder.add(Some(item), "curve", None);
    for _ in 0..=MAX_CURVE_KEYS {
        builder.add(Some(curve), "key", None);
    }
    assert!(preflight(&builder.snapshot(), NodeId(1), true).is_err());
}

#[test]
fn preflight_includes_curve_script_bytes_before_cloning() {
    let mut builder = SnapshotBuilder::new();
    let item = builder.add(Some(NodeId(1)), "item", None);
    let curve = builder.add(Some(item), "curve", None);
    let key = builder.add(Some(curve), "key", None);
    let easing = builder.add(Some(key), "easing", None);
    builder.add(
        Some(easing),
        "script_source",
        Some(ParamValue::Str("x".repeat(MAX_CONFIG_WEIGHT / 2))),
    );
    assert!(preflight(&builder.snapshot(), NodeId(1), true).is_err());
}

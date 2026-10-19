use super::*;
use crate::edit::Edit;
use crate::node::{Node, NodeId};
use crate::parameter::ParamValue;
use crate::process_ctx::ProcessCtx;

#[crate::node("opaque_authored_probe")]
struct OpaqueAuthoredProbe {
    #[state(default = 0, persist)]
    count: i32,
    runtime_calls: usize,
}

#[crate::node("opaque_authored_probe", from_struct)]
impl Node for OpaqueAuthoredProbe {
    fn engine_set_script_property(
        &mut self,
        _ctx: &mut ProcessCtx,
        property: &str,
        value: &ParamValue,
    ) -> Result<bool, String> {
        if property != "count" {
            return Ok(false);
        }
        self.count = value.as_int().ok_or("count expects an integer")?;
        self.runtime_calls += 1;
        Ok(true)
    }

    fn engine_call_script_method(
        &mut self,
        _ctx: &mut ProcessCtx,
        method: &str,
        _args: &[ParamValue],
    ) -> Result<bool, String> {
        if method != "increment" {
            return Ok(false);
        }
        self.count += 1;
        self.runtime_calls += 1;
        Ok(true)
    }
}

crate::define_node_enum!(
    enum OpaqueTestNode {
        OpaqueAuthoredProbe,
        ComposedAuthoredProbe,
        NestedReadyAuthoredProbe,
        AutoAuthoredProbe,
    }
);

#[crate::node("composed_authored_probe")]
struct ComposedAuthoredProbe {
    base: OpaqueAuthoredProbe,
    #[state(default = 0, persist)]
    own: i32,
}

#[crate::node("composed_authored_probe", via = base, from_struct)]
impl Node for ComposedAuthoredProbe {}

#[crate::node("nested_ready_authored_probe")]
struct NestedReadyAuthoredProbe {}

#[crate::node("nested_ready_authored_probe", from_struct)]
impl Node for NestedReadyAuthoredProbe {
    fn on_node_ready(&mut self, ctx: &mut ProcessCtx, _context: crate::node::NodeCreationContext) {
        let root = ctx.tree_snapshot().unwrap().root();
        ctx.call_node_mutation_without_snapshot(root, move |node, ctx| {
            node.as_any_mut().downcast_mut::<OpaqueAuthoredProbe>().unwrap().count = 9;
            ctx.call_node_mutation_without_snapshot(root, |node, _| {
                node.as_any_mut().downcast_mut::<OpaqueAuthoredProbe>().unwrap().count = 4;
                Ok(())
            });
            Ok(())
        });
    }
}

#[crate::node("auto_authored_probe", impl_node)]
struct AutoAuthoredProbe {
    #[state(default = 0, persist)]
    count: i32,
}

#[test]
fn opaque_auto_implemented_node_preserves_authored_defaults_in_history() {
    let mut engine = Engine::new(OpaqueTestNode::from(AutoAuthoredProbe::new()));
    let before = engine.to_project_json_with(|node| node.project_encode_data()).unwrap();
    engine.edits.push(Edit::CallNodeMutation {
        node: engine.root,
        needs_tree_snapshot: false,
        callback: Box::new(|node, _| {
            node.as_any_mut().downcast_mut::<AutoAuthoredProbe>().unwrap().count = 3;
            Ok(())
        }),
    });
    engine.apply_edits().unwrap();
    let after = engine.to_project_json_with(|node| node.project_encode_data()).unwrap();
    assert_ne!(before, after);
    assert!(engine.undo().unwrap());
    assert_eq!(
        engine.to_project_json_with(|node| node.project_encode_data()).unwrap(),
        before
    );
    assert!(engine.redo().unwrap());
    assert_eq!(
        engine.to_project_json_with(|node| node.project_encode_data()).unwrap(),
        after
    );
}

fn rust_edit(node: NodeId, reject: bool) -> Edit {
    Edit::CallNodeMutation {
        node,
        needs_tree_snapshot: false,
        callback: Box::new(move |node, _ctx| {
            let target = node.as_any_mut().downcast_mut::<OpaqueAuthoredProbe>().unwrap();
            target.count = 9;
            target.runtime_calls += 1;
            if reject {
                Err("rejected after mutation".into())
            } else {
                Ok(())
            }
        }),
    }
}

#[test]
fn opaque_authored_edits_restore_defaults_on_undo_and_replay_without_calling_again() {
    for kind in ["rust", "property", "method"] {
        let mut engine = Engine::new(OpaqueTestNode::from(OpaqueAuthoredProbe::new(0)));
        let root = engine.root;
        let before = engine.to_project_json_with(|node| node.project_encode_data()).unwrap();
        engine.edits.push(match kind {
            "rust" => rust_edit(root, false),
            "property" => Edit::SetNodeScriptProperty {
                node: root,
                write: crate::script::ScriptPropertyWrite::prepare("count", ParamValue::Int(5)).unwrap(),
            },
            _ => Edit::CallNodeScriptMethod {
                node: root,
                call: crate::script::ScriptMethodCall::prepare("increment", Vec::new()).unwrap(),
            },
        });
        engine.apply_edits().unwrap();
        let after = engine.to_project_json_with(|node| node.project_encode_data()).unwrap();
        assert_ne!(before, after, "{kind}");
        assert_eq!(engine.undo_len(), 1, "{kind}");
        assert!(engine.undo().unwrap());
        assert_eq!(
            engine.to_project_json_with(|node| node.project_encode_data()).unwrap(),
            before,
            "{kind}"
        );
        assert!(engine.redo().unwrap());
        assert_eq!(
            engine.to_project_json_with(|node| node.project_encode_data()).unwrap(),
            after,
            "{kind}"
        );
        let target = engine
            .nodes
            .get(root)
            .unwrap()
            .as_any()
            .downcast_ref::<OpaqueAuthoredProbe>()
            .unwrap();
        assert_eq!(target.runtime_calls, 1, "{kind}");
    }
}

#[test]
fn opaque_authored_failure_restores_atomic_graph_history_and_publication() {
    for reject_callback in [false, true] {
        let mut engine = Engine::new(OpaqueTestNode::from(OpaqueAuthoredProbe::new(0)));
        let root = engine.root;
        let before = engine.to_project_json_with(|node| node.project_encode_data()).unwrap();
        let history = engine.ui_history_state();
        let events = engine.ui_event_log().to_vec();
        let dirty = engine.project_dirty_nodes.clone();
        let result = engine.apply_atomic_edit_operation(|engine| {
            engine.edits.push(rust_edit(root, reject_callback));
            engine.edits.push(Edit::SetParam {
                node: NodeId(u64::MAX),
                value: ParamValue::Int(1),
                behaviour: Default::default(),
            });
            engine.apply_edits()
        });
        assert!(result.is_err());
        assert_eq!(
            engine.to_project_json_with(|node| node.project_encode_data()).unwrap(),
            before
        );
        assert_eq!(engine.ui_history_state(), history);
        assert_eq!(engine.ui_event_log(), events);
        assert_eq!(engine.project_dirty_nodes, dirty);
        engine.run_tick(std::time::Duration::from_millis(1)).unwrap();
    }
}

#[test]
fn opaque_nested_edits_keep_authored_order_in_one_prefix_transaction() {
    let mut engine = Engine::new(OpaqueTestNode::from(OpaqueAuthoredProbe::new(0)));
    let root = engine.root;
    let before = engine.to_project_json_with(|node| node.project_encode_data()).unwrap();
    engine.add_node(NestedReadyAuthoredProbe::new().into(), None);
    engine.apply_edits().unwrap();
    let after = engine.to_project_json_with(|node| node.project_encode_data()).unwrap();
    assert_eq!(engine.undo_len(), 1);
    assert!(engine.undo().unwrap());
    assert_eq!(
        engine.to_project_json_with(|node| node.project_encode_data()).unwrap(),
        before
    );
    assert!(engine.redo().unwrap());
    assert_eq!(
        engine.to_project_json_with(|node| node.project_encode_data()).unwrap(),
        after
    );
    assert_eq!(
        engine
            .nodes
            .get(root)
            .unwrap()
            .as_any()
            .downcast_ref::<OpaqueAuthoredProbe>()
            .unwrap()
            .count,
        4
    );
}

#[test]
fn opaque_rejected_callback_retains_undo_for_its_committed_self_change() {
    let mut engine = Engine::new(OpaqueTestNode::from(OpaqueAuthoredProbe::new(0)));
    let before = engine.to_project_json_with(|node| node.project_encode_data()).unwrap();
    engine.edits.push(rust_edit(engine.root, true));
    assert!(matches!(
        engine.apply_edits(),
        Err(EngineEditError::NodeMutationRejected { .. })
    ));
    assert_eq!(engine.undo_len(), 1);
    assert!(engine.undo().unwrap());
    assert_eq!(
        engine.to_project_json_with(|node| node.project_encode_data()).unwrap(),
        before
    );
}

#[test]
fn opaque_parameter_configuration_restores_full_defaults_and_cache() {
    use crate::parameter::{Parameter, ParameterChangeCheck};
    let mut engine = Engine::new(Parameter::new(
        "Value",
        ParamValue::Float(0.0),
        ParameterChangeCheck::ValueChange,
    ));
    let before = engine.to_project_json_with(|node| node.project_encode_data()).unwrap();
    engine.edits.push(Edit::CallNodeMutation {
        node: engine.root,
        needs_tree_snapshot: false,
        callback: Box::new(|node, _| {
            let param = node.as_any_mut().downcast_mut::<Parameter>().unwrap();
            param.read_only = true;
            param.value = ParamValue::Float(0.75);
            Ok(())
        }),
    });
    engine.apply_edits().unwrap();
    assert_eq!(
        engine.process_tree_snapshot().node(engine.root).unwrap().param_value,
        Some(ParamValue::Float(0.75))
    );
    assert!(engine.undo().unwrap());
    assert_eq!(
        engine.to_project_json_with(|node| node.project_encode_data()).unwrap(),
        before
    );
    assert!(engine.redo().unwrap());
    assert!(engine.nodes.get(engine.root).unwrap().read_only);
    assert_eq!(
        engine.process_tree_snapshot().node(engine.root).unwrap().param_value,
        Some(ParamValue::Float(0.75))
    );
}

#[test]
fn opaque_composed_authored_state_restores_both_layers_without_runtime_reset() {
    let mut engine = Engine::new(OpaqueTestNode::from(ComposedAuthoredProbe::new(
        OpaqueAuthoredProbe::new(0),
    )));
    let before = engine.to_project_json_with(|node| node.project_encode_data()).unwrap();
    engine.edits.push(Edit::CallNodeMutation {
        node: engine.root,
        needs_tree_snapshot: false,
        callback: Box::new(|node, _| {
            let target = node.as_any_mut().downcast_mut::<ComposedAuthoredProbe>().unwrap();
            target.base.count = 2;
            target.base.runtime_calls = 7;
            target.own = 3;
            target.node_data_mut().user_role = crate::node::UserNodeRole::ItemRoot;
            Ok(())
        }),
    });
    engine.apply_edits().unwrap();
    let after = engine.to_project_json_with(|node| node.project_encode_data()).unwrap();
    assert!(engine.undo().unwrap());
    assert_eq!(
        engine.to_project_json_with(|node| node.project_encode_data()).unwrap(),
        before
    );
    assert!(engine.redo().unwrap());
    assert_eq!(
        engine.to_project_json_with(|node| node.project_encode_data()).unwrap(),
        after
    );
    let target = engine
        .nodes
        .get(engine.root)
        .unwrap()
        .as_any()
        .downcast_ref::<ComposedAuthoredProbe>()
        .unwrap();
    assert_eq!(target.base.runtime_calls, 7);
}

#[test]
fn opaque_callback_cannot_bypass_engine_identity_and_topology_ownership() {
    let mut engine = Engine::new(OpaqueTestNode::from(OpaqueAuthoredProbe::new(0)));
    let root = engine.root;
    let uuid = engine.nodes.get(root).unwrap().node_data().meta.uuid;
    let before = engine.to_project_json_with(|node| node.project_encode_data()).unwrap();
    engine.edits.push(Edit::CallNodeMutation {
        node: root,
        needs_tree_snapshot: false,
        callback: Box::new(|node, _| {
            node.node_data_mut().parent = Some(NodeId(u64::MAX));
            node.node_data_mut().meta.uuid = crate::node::NodeUuid::nil();
            Ok(())
        }),
    });
    assert!(matches!(
        engine.apply_edits(),
        Err(EngineEditError::NodeMutationRejected { .. })
    ));
    assert_eq!(
        engine.to_project_json_with(|node| node.project_encode_data()).unwrap(),
        before
    );
    assert_eq!(engine.node_id_by_uuid(uuid), Some(root));
    assert_eq!(engine.undo_len(), 0);
}

use super::*;
use crate::edit::Edit;
use crate::events::CustomEvent;
use crate::node::{Folder, Node, NodeId, NodeMetaPatch};
use crate::parameter::{
    ParamValue, Parameter, ParameterChangeCheck, ParameterConstraints, ParameterControlMode, ParameterControlSpec,
    ParameterControlState, RangeConstraint,
};
use crate::process_ctx::ProcessCtx;
use crate::ui_sync::{UiEditIntent, UiParameterControlStateDto};

#[crate::node("direct_intent_probe")]
struct DirectIntentProbe {
    failure: String,
    #[state(default = 0, persist)]
    authored_changes: i32,
}

#[crate::node("direct_intent_probe", from_struct)]
impl Node for DirectIntentProbe {
    fn on_param_change(&mut self, ctx: &mut ProcessCtx, _param: NodeId, _old: ParamValue) {
        if self.failure == "parameter" {
            self.reject(ctx);
        }
    }
    fn on_param_constraints_changed(
        &mut self,
        ctx: &mut ProcessCtx,
        _param: NodeId,
        _old: ParameterConstraints,
        _new: ParameterConstraints,
    ) {
        if self.failure == "constraints" {
            self.reject(ctx);
        }
    }
    fn on_param_control_changed(
        &mut self,
        ctx: &mut ProcessCtx,
        _param: NodeId,
        _old: ParameterControlState,
        _new: ParameterControlState,
    ) {
        if self.failure == "control" {
            self.reject(ctx);
        }
    }
    fn on_effective_enabled_changed(&mut self, ctx: &mut ProcessCtx, enabled: bool) {
        if self.failure == "metadata" && !enabled {
            self.reject(ctx);
        }
    }
    fn on_child_moved(&mut self, ctx: &mut ProcessCtx, _child: NodeId, _old: NodeId, _new: NodeId) {
        if self.failure == "move" {
            self.reject(ctx);
        }
    }
    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: CustomEvent) {
        if self.failure == "event" && event.topic == "reject" {
            self.reject(ctx);
        }
    }
}

impl DirectIntentProbe {
    fn reject(&mut self, ctx: &mut ProcessCtx) {
        self.authored_changes += 1;
        ctx.patch_node_meta(
            self.id(),
            NodeMetaPatch {
                label: Some("Rejected direct intent".into()),
                ..Default::default()
            },
        );
        ctx.set_param(NodeId(u64::MAX), ParamValue::Int(1));
    }
}

crate::define_node_enum!(
    enum DirectIntentNode {
        DirectIntentProbe,
    }
);

#[test]
fn rejected_direct_intents_restore_graph_history_publication_and_next_tick() {
    for kind in ["parameter", "constraints", "control", "metadata", "move", "event"] {
        for active_session in [false, true] {
            let mut engine = Engine::new(DirectIntentNode::from(DirectIntentProbe::new(kind.into())));
            let root = engine.root;
            engine.add_node(
                Parameter::new("Value", ParamValue::Float(0.5), ParameterChangeCheck::ValueChange).into(),
                None,
            );
            engine.add_node(Folder::new("Destination").into(), None);
            engine.edits.push(Edit::AddEventListener {
                subscriber: root,
                subscription: crate::node::EventSubscription::subtree(root, u32::MAX),
            });
            engine.apply_edits().unwrap();
            let param = engine.nodes.get(root).unwrap().node_data().first_child.unwrap();
            let destination = engine.nodes.get(param).unwrap().node_data().next_sibling.unwrap();
            engine.run_tick(std::time::Duration::from_millis(1)).unwrap();
            engine.clear_history();
            engine.edits.push(Edit::PatchMeta {
                node: root,
                patch: NodeMetaPatch {
                    label: Some("Previous".into()),
                    ..Default::default()
                },
            });
            engine.apply_edits().unwrap();
            engine.undo().unwrap();
            if active_session {
                assert!(
                    engine
                        .apply_ui_intent(UiEditIntent::BeginEdit {
                            client_edit_id: "outer".into(),
                            label: None
                        })
                        .success
                );
            }
            let before = engine.to_project_json_with(|node| node.project_encode_data()).unwrap();
            let history = engine.ui_history_state();
            let events = engine.ui_event_log().to_vec();
            let dirty = engine.project_dirty_nodes.clone();
            let graph_version = engine.ui_graph_version;
            let intent = match kind {
                "parameter" => UiEditIntent::SetParam {
                    node: param,
                    value: ParamValue::Float(0.75),
                    behaviour: Default::default(),
                },
                "constraints" => UiEditIntent::SetParamConstraints {
                    node: param,
                    constraints: ParameterConstraints {
                        range: RangeConstraint::uniform(Some(0.0), Some(1.0)),
                        ..Default::default()
                    },
                },
                "control" => UiEditIntent::SetParamControlState {
                    node: param,
                    state: UiParameterControlStateDto {
                        mode: ParameterControlMode::Expression,
                        spec: ParameterControlSpec::Expression,
                    },
                },
                "metadata" => UiEditIntent::PatchMeta {
                    node: root,
                    patch: NodeMetaPatch {
                        enabled: Some(false),
                        ..Default::default()
                    }
                    .into(),
                },
                "move" => UiEditIntent::MoveNode {
                    node: param,
                    new_parent: destination,
                    new_prev_sibling: None,
                },
                _ => UiEditIntent::SendNodeEvent {
                    node: root,
                    topic: "reject".into(),
                    payload: serde_json::json!({}),
                },
            };
            let ack = engine.apply_ui_intent(intent);
            assert!(!ack.success, "{kind} unexpectedly succeeded");
            assert_eq!(
                engine.to_project_json_with(|node| node.project_encode_data()).unwrap(),
                before,
                "{kind}"
            );
            assert_eq!(engine.ui_history_state(), history, "{kind}");
            assert_eq!(ack.history, history, "{kind}");
            assert_eq!(engine.ui_event_log(), events, "{kind}");
            assert_eq!(engine.project_dirty_nodes, dirty, "{kind}");
            assert_eq!(engine.ui_graph_version, graph_version, "{kind}");
            engine.run_tick(std::time::Duration::from_millis(1)).unwrap();
            if active_session {
                assert!(
                    engine
                        .apply_ui_intent(UiEditIntent::EndEdit {
                            client_edit_id: "outer".into()
                        })
                        .success
                );
            }
            assert!(engine.redo().unwrap());
        }
    }
}

#[test]
fn direct_control_intent_preserves_exact_tree_and_identities_through_history() {
    for (mode, spec) in [
        (ParameterControlMode::Expression, ParameterControlSpec::Expression),
        (ParameterControlMode::Animation, ParameterControlSpec::Animation),
    ] {
        let mut engine = Engine::new(DirectIntentNode::from(Folder::new("Root")));
        engine.add_node(
            Parameter::new("Value", ParamValue::Float(0.5), ParameterChangeCheck::ValueChange).into(),
            None,
        );
        engine.apply_edits().unwrap();
        let param = engine.nodes.get(engine.root).unwrap().node_data().first_child.unwrap();
        engine.clear_history();
        let before = engine.to_project_json_with(|node| node.project_encode_data()).unwrap();
        let ack = engine.apply_ui_intent(UiEditIntent::SetParamControlState {
            node: param,
            state: UiParameterControlStateDto { mode, spec },
        });
        assert!(ack.success, "{:?}", ack.error_message);
        let after = engine.to_project_json_with(|node| node.project_encode_data()).unwrap();
        for _ in 0..2 {
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
    }
}

#[test]
fn rejected_direct_event_restores_existing_user_context_aliases_and_lookup_cache() {
    use crate::edit::NodeTree;
    use crate::node::UserContextNode;
    let mut engine = Engine::new(DirectIntentNode::from(DirectIntentProbe::new("event".into())));
    let root = engine.root;
    engine.edits.push(Edit::AddNodeTree {
        tree: NodeTree::new(UserContextNode::new("Context")).with_child(NodeTree::new(Parameter::new(
            "Aliased",
            ParamValue::Float(0.5),
            ParameterChangeCheck::ValueChange,
        ))),
        parent: root,
        prev_sibling: None,
    });
    engine.apply_edits().unwrap();
    engine.run_tick(std::time::Duration::from_millis(1)).unwrap();
    let scope = engine.nodes.get(root).unwrap().node_data().first_child.unwrap();
    let param = engine.nodes.get(scope).unwrap().node_data().first_child.unwrap();
    engine.upsert_user_context_entry(root, "stable_alias", param).unwrap();
    let before = serde_json::to_value(engine.ui_user_contexts()).unwrap();
    let lookup = engine.resolve_user_context_symbol(root, "stable_alias", None);
    assert!(
        !engine
            .apply_ui_intent(UiEditIntent::SendNodeEvent {
                node: root,
                topic: "reject".into(),
                payload: serde_json::json!({})
            })
            .success
    );
    assert_eq!(serde_json::to_value(engine.ui_user_contexts()).unwrap(), before);
    assert_eq!(engine.resolve_user_context_symbol(root, "stable_alias", None), lookup);
}

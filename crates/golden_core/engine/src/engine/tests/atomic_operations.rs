use super::*;
use crate::edit::Edit;
use crate::node::{Folder, Node, NodeId, NodeMetaPatch};
use crate::parameter::ParamValue;
use crate::process_ctx::ProcessCtx;
use crate::ui_sync::UiEditIntent;

#[crate::node("atomic_operation_root")]
struct AtomicOperationRoot {}

#[crate::node("atomic_operation_root", from_struct)]
impl Node for AtomicOperationRoot {
    fn on_child_removed(&mut self, ctx: &mut ProcessCtx, _parent: NodeId, _child: NodeId) {
        ctx.patch_node_meta(
            self.id(),
            NodeMetaPatch {
                label: Some("Leaked removal callback".into()),
                ..Default::default()
            },
        );
        ctx.set_param(NodeId(u64::MAX), ParamValue::Int(1));
    }
}

crate::define_node_enum!(
    enum AtomicOperationNode {
        AtomicOperationRoot,
    }
);

#[test]
fn rejected_remove_stabilization_restores_graph_publication_and_enclosing_history() {
    for active_session in [false, true] {
        let mut engine = Engine::new(AtomicOperationNode::from(AtomicOperationRoot::new()));
        engine.add_node(Folder::new("Existing").into(), None);
        engine.apply_edits().unwrap();
        let child = engine.nodes.get(engine.root).unwrap().node_data().first_child.unwrap();
        let uuid = engine.nodes.get(child).unwrap().node_data().meta.uuid;
        engine.run_tick(std::time::Duration::from_millis(1)).unwrap();
        engine.clear_history();
        assert!(
            engine
                .apply_ui_intent(UiEditIntent::PatchMeta {
                    node: engine.root,
                    patch: NodeMetaPatch {
                        label: Some("Previous accepted edit".into()),
                        ..Default::default()
                    }
                    .into(),
                })
                .success
        );
        engine.undo().unwrap();
        if active_session {
            assert!(
                engine
                    .apply_ui_intent(UiEditIntent::BeginEdit {
                        client_edit_id: "outer".into(),
                        label: Some("Outer session".into()),
                    })
                    .success
            );
            assert!(
                engine
                    .apply_ui_intent(UiEditIntent::PatchMeta {
                        node: engine.root,
                        patch: NodeMetaPatch {
                            label: Some("Accepted within outer session".into()),
                            ..Default::default()
                        }
                        .into(),
                    })
                    .success
            );
        }
        let before = engine.to_project_json_with(|node| node.project_encode_data()).unwrap();
        let history = engine.ui_history_state();
        let events = engine.ui_event_log().to_vec();
        let dirty = engine.project_dirty_nodes.clone();
        let graph_version = engine.ui_graph_version;
        let ack = engine.apply_ui_intent(UiEditIntent::RemoveNode { node: child });
        assert!(!ack.success);
        assert_eq!(
            engine.to_project_json_with(|node| node.project_encode_data()).unwrap(),
            before
        );
        assert_eq!(engine.ui_history_state(), history);
        assert_eq!(engine.ui_event_log(), events);
        assert_eq!(engine.project_dirty_nodes, dirty);
        assert_eq!(engine.ui_graph_version, graph_version);
        assert_eq!(engine.node_id_by_uuid(uuid), Some(child));
        assert_eq!(ack.history, history);
        assert!(engine.edits.pending.is_empty());
        engine.run_tick(std::time::Duration::from_millis(1)).unwrap();
        if active_session {
            assert!(
                engine
                    .apply_ui_intent(UiEditIntent::EndEdit {
                        client_edit_id: "outer".into()
                    })
                    .success
            );
            assert!(engine.undo().unwrap());
        } else {
            assert!(engine.redo().unwrap());
        }
        assert!(engine.nodes.contains(child));
    }
}

#[test]
fn atomic_operation_keeps_prior_queued_work_outside_its_commit_or_rollback() {
    for reject in [false, true] {
        let mut engine = Engine::new(Folder::new("Root"));
        engine.edits.push(Edit::PatchMeta {
            node: engine.root,
            patch: NodeMetaPatch {
                label: Some("Previously queued rename".into()),
                ..Default::default()
            },
        });
        engine.emit_inbox_event(crate::events::EventKind::Custom(crate::events::CustomEvent::new(
            "previous_inbox",
            Some(engine.root),
            serde_json::json!({}),
        )));
        let previous_inbox = engine.inbox.clone();
        let result: Result<(), EngineEditError> = engine.apply_atomic_edit_operation(|engine| {
            assert!(engine.edits.pending.is_empty());
            assert!(engine.inbox.events.is_empty());
            engine.add_node(Folder::new("Candidate"), None);
            engine.apply_edits()?;
            if reject {
                engine.edits.push(Edit::SetParam {
                    node: engine.root,
                    value: ParamValue::Int(1),
                    behaviour: crate::parameter::ParameterEventBehaviour::default(),
                });
                engine.apply_edits()?;
            }
            Ok(())
        });
        assert_eq!(result.is_err(), reject);
        assert_eq!(engine.undo_len(), usize::from(!reject));
        assert_eq!(engine.nodes.len(), if reject { 1 } else { 2 });
        assert_eq!(engine.edits.pending.len(), 1);
        assert!(engine.inbox.events.starts_with(&previous_inbox.events));
        if reject {
            assert_eq!(engine.inbox, previous_inbox);
        }
        assert_eq!(engine.nodes.get(engine.root).unwrap().node_data().meta.label, "Root");
        engine.apply_edits().unwrap();
        assert_eq!(
            engine.nodes.get(engine.root).unwrap().node_data().meta.label,
            "Previously queued rename"
        );
    }
}

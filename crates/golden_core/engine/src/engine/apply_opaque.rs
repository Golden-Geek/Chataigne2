use crate::edit::NodeMutation;
use crate::node::{Node, NodeAuthoredData, NodeData, NodeId, NodeMeta, NodeUuid, UserNodeRole};
use crate::parameter::ParameterSnapshot;
use crate::process_ctx::{ExecutionPhase, ProcessCtx};
use crate::ui_sync::UiGraphOp;

use super::apply_tree::AppliedEdit;
use super::{Engine, EngineEditError};

#[derive(Clone, PartialEq)]
pub(crate) struct AuthoredNodeState {
    meta: NodeMeta,
    user_role: UserNodeRole,
    data: NodeAuthoredData,
}

#[derive(Clone, Copy, PartialEq)]
struct NodeBinding {
    id: NodeId,
    parent: Option<NodeId>,
    first_child: Option<NodeId>,
    last_child: Option<NodeId>,
    prev_sibling: Option<NodeId>,
    next_sibling: Option<NodeId>,
    uuid: NodeUuid,
    effective_enabled: bool,
}

impl NodeBinding {
    fn capture(data: &NodeData) -> Self {
        Self {
            id: data.id,
            parent: data.parent,
            first_child: data.first_child,
            last_child: data.last_child,
            prev_sibling: data.prev_sibling,
            next_sibling: data.next_sibling,
            uuid: data.meta.uuid,
            effective_enabled: data.effective_enabled,
        }
    }

    fn restore(self, data: &mut NodeData) {
        data.id = self.id;
        data.parent = self.parent;
        data.first_child = self.first_child;
        data.last_child = self.last_child;
        data.prev_sibling = self.prev_sibling;
        data.next_sibling = self.next_sibling;
        data.meta.uuid = self.uuid;
        data.effective_enabled = self.effective_enabled;
    }
}

pub(crate) struct AuthoredNodeEffect {
    pub(crate) node: NodeId,
    pub(crate) old: AuthoredNodeState,
    pub(crate) new: AuthoredNodeState,
}

impl AuthoredNodeState {
    pub(super) fn capture(node: &impl Node) -> Result<Self, String> {
        Ok(Self {
            meta: node.node_data().meta.clone(),
            user_role: node.node_data().user_role,
            data: node.engine_capture_authored_data()?,
        })
    }
}

impl<T: Node> Engine<T> {
    /// Records self-state changed by event/lifecycle handlers inside an authored edit.
    /// Runtime-only dispatch avoids serialization; candidate state belongs to its insertion.
    pub(super) fn run_authored_callback(
        &mut self,
        node: NodeId,
        ctx: &mut ProcessCtx,
        callback: impl FnOnce(&mut T, &mut ProcessCtx),
    ) -> Result<(), EngineEditError> {
        if self.history_replay {
            return self.run_runtime_callback(node, ctx, callback);
        }
        let Some(target) = self.nodes.get(node) else {
            return Ok(());
        };
        let node_type = target.get_type().to_owned();
        let reject = |message| EngineEditError::NodeMutationRejected {
            edit_index: 0,
            operation: "AuthoredCallback",
            node,
            node_type: node_type.clone(),
            message,
        };
        let binding = NodeBinding::capture(target.node_data());
        let capture = self.capture_opaque_history(false, node);
        let old = if capture {
            Some(AuthoredNodeState::capture(target).map_err(&reject)?)
        } else {
            None
        };
        let old_param = capture.then(|| target.engine_param_snapshot()).flatten();
        let order = capture.then(|| self.edit_order().expect("authored callback has a journal"));
        crate::logger::with_node_origin(node, || {
            callback(self.nodes.get_mut(node).expect("validated callback target"), ctx)
        });
        let data = self
            .nodes
            .get_mut(node)
            .expect("callback target remains live")
            .node_data_mut();
        let invalid_binding = NodeBinding::capture(data) != binding;
        if invalid_binding {
            binding.restore(data);
        }
        if let Some(old) = old {
            let new = match AuthoredNodeState::capture(self.nodes.get(node).expect("callback target remains live")) {
                Ok(new) => new,
                Err(error) => {
                    let original = reject(error);
                    return match self.restore_authored_node_state(node, &old) {
                        Ok(()) => Err(original),
                        Err(rollback) => Err(EngineEditError::AtomicRollbackFailed {
                            original: Box::new(original),
                            rollback: Box::new(rollback),
                        }),
                    };
                }
            };
            if old != new {
                self.capture_journal_step(
                    order.expect("capture order reserved"),
                    AuthoredNodeEffect { node, old, new }.into(),
                );
                self.project_dirty_nodes.insert(node);
                self.publish_authored_node_change(node, old_param, None)?;
            }
        }
        if invalid_binding {
            return Err(reject(
                "callbacks must queue identity and topology edits through ProcessCtx".into(),
            ));
        }
        Ok(())
    }

    pub(crate) fn apply_set_node_script_property(
        &mut self,
        edit_index: usize,
        node: NodeId,
        write: crate::script::ScriptPropertyWrite,
        capture: bool,
    ) -> Result<AppliedEdit<Option<AuthoredNodeEffect>>, EngineEditError> {
        self.apply_opaque_edit(
            edit_index,
            "SetNodeScriptProperty",
            node,
            capture,
            false,
            None,
            move |target, ctx| {
                let property = write.property();
                target
                    .engine_set_script_property(ctx, property, write.value())
                    .and_then(|handled| {
                        handled
                            .then_some(())
                            .ok_or_else(|| format!("property '{property}' is not exposed by target node"))
                    })
            },
            |message, node_type| EngineEditError::ScriptPropertyRejected {
                edit_index,
                operation: "SetNodeScriptProperty",
                node,
                node_type,
                message,
            },
        )
    }

    pub(crate) fn apply_call_node_script_method(
        &mut self,
        edit_index: usize,
        node: NodeId,
        call: crate::script::ScriptMethodCall,
        capture: bool,
    ) -> Result<AppliedEdit<Option<AuthoredNodeEffect>>, EngineEditError> {
        self.apply_opaque_edit(
            edit_index,
            "CallNodeScriptMethod",
            node,
            capture,
            true,
            None,
            move |target, ctx| {
                let method = call.method();
                ctx.set_script_args(Some(call.args().clone()));
                let result = target
                    .engine_call_script_method(ctx, method, call.args())
                    .and_then(|handled| {
                        handled
                            .then_some(())
                            .ok_or_else(|| format!("method '{method}' is not exposed by target node"))
                    });
                ctx.set_script_args(None);
                result
            },
            |message, node_type| EngineEditError::ScriptMethodRejected {
                edit_index,
                operation: "CallNodeScriptMethod",
                node,
                node_type,
                message,
            },
        )
    }

    pub(crate) fn apply_call_node_mutation(
        &mut self,
        edit_index: usize,
        node: NodeId,
        callback: NodeMutation,
        needs_tree_snapshot: bool,
        capture: bool,
        source: Option<NodeId>,
    ) -> Result<AppliedEdit<Option<AuthoredNodeEffect>>, EngineEditError> {
        self.apply_opaque_edit(
            edit_index,
            "CallNodeMutation",
            node,
            capture,
            needs_tree_snapshot,
            source,
            move |target, ctx| callback(target, ctx),
            |message, node_type| EngineEditError::NodeMutationRejected {
                edit_index,
                operation: "CallNodeMutation",
                node,
                node_type,
                message,
            },
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn apply_opaque_edit(
        &mut self,
        edit_index: usize,
        operation: &'static str,
        node: NodeId,
        capture: bool,
        snapshot: bool,
        source: Option<NodeId>,
        callback: impl FnOnce(&mut T, &mut ProcessCtx) -> Result<(), String>,
        reject: impl Fn(String, String) -> EngineEditError,
    ) -> Result<AppliedEdit<Option<AuthoredNodeEffect>>, EngineEditError> {
        let target = self.nodes.get(node).ok_or(EngineEditError::NodeNotFound {
            edit_index,
            operation,
            node,
        })?;
        let node_type = target.get_type().to_string();
        let binding = NodeBinding::capture(target.node_data());
        let old = if capture {
            Some(AuthoredNodeState::capture(target).map_err(|error| reject(error, node_type.clone()))?)
        } else {
            None
        };
        let old_param = capture.then(|| target.engine_param_snapshot()).flatten();
        let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, self.time);
        ctx.set_node_wake_source(self.node_wakes.source.clone(), node);
        ctx.runtime_elapsed = self.runtime_elapsed;
        if snapshot {
            ctx.set_tree_snapshot(self.get_or_build_tick_snapshot());
        }
        let mut body = callback(self.nodes.get_mut(node).expect("validated target"), &mut ctx)
            .map_err(|error| reject(error, node_type.clone()));
        let data = self
            .nodes
            .get_mut(node)
            .expect("callback target remains live")
            .node_data_mut();
        if NodeBinding::capture(data) != binding {
            binding.restore(data);
            body = Err(reject(
                "callbacks must queue identity and topology edits through ProcessCtx".into(),
                node_type.clone(),
            ));
        }
        let effect = if let Some(old) = old {
            let new = match AuthoredNodeState::capture(self.nodes.get(node).expect("callback target remains live")) {
                Ok(new) => new,
                Err(error) => {
                    let original = reject(
                        format!("failed to capture authored callback result: {error}"),
                        node_type,
                    );
                    return match self.restore_authored_node_state(node, &old) {
                        Ok(()) => Err(original),
                        Err(rollback) => Err(EngineEditError::AtomicRollbackFailed {
                            original: Box::new(original),
                            rollback: Box::new(rollback),
                        }),
                    };
                }
            };
            (old != new).then_some(AuthoredNodeEffect { node, old, new })
        } else {
            None
        };
        let publication = if effect.is_some() {
            self.publish_authored_node_change(node, old_param, source)
        } else {
            if !capture {
                // Uncaptured native callbacks can write state without semantic events.
                // Conservatively invalidate their subtree before the next reader runs.
                self.mark_tree_changed(node);
            }
            Ok(())
        };
        // Capture self changes before absorbing nested requests. A completion error
        // still returns the committed effect to the normal history/journal finalizer.
        let completion = body.and(publication).and_then(|()| self.absorb_edits(&mut ctx));
        Ok(AppliedEdit { effect, completion })
    }

    pub(crate) fn restore_authored_node_state(
        &mut self,
        node: NodeId,
        state: &AuthoredNodeState,
    ) -> Result<(), EngineEditError> {
        let old_param = self.nodes.get(node).and_then(Node::engine_param_snapshot);
        self.restore_authored_node_state_silent(node, state.clone())?;
        self.publish_authored_node_change(node, old_param, None)
    }

    pub(super) fn restore_authored_node_state_silent(
        &mut self,
        node: NodeId,
        state: AuthoredNodeState,
    ) -> Result<(), EngineEditError> {
        let target = self.nodes.get_mut(node).ok_or(EngineEditError::NodeNotFound {
            edit_index: 0,
            operation: "RestoreAuthoredState",
            node,
        })?;
        let node_type = target.get_type().to_string();
        target
            .engine_restore_authored_data(state.data)
            .map_err(|message| EngineEditError::NodeMutationRejected {
                edit_index: 0,
                operation: "RestoreAuthoredState",
                node,
                node_type,
                message,
            })?;
        self.unregister_node_uuid(node);
        self.nodes.get_mut(node).expect("restored target").node_data_mut().meta = state.meta;
        self.nodes
            .get_mut(node)
            .expect("restored target")
            .node_data_mut()
            .user_role = state.user_role;
        self.register_node_uuid(node);
        self.populate_param_cache_entry(node);
        self.clear_tick_tree_snapshot();
        Ok(())
    }

    /// Activation/teardown must preserve authored data retained for history.
    pub(super) fn run_runtime_callback(
        &mut self,
        node: NodeId,
        ctx: &mut ProcessCtx,
        callback: impl FnOnce(&mut T, &mut ProcessCtx),
    ) -> Result<(), EngineEditError> {
        let Some(target) = self.nodes.get(node) else {
            return Ok(());
        };
        let state = AuthoredNodeState::capture(target).map_err(|message| EngineEditError::NodeMutationRejected {
            edit_index: 0,
            operation: "HistoryCallback",
            node,
            node_type: target.get_type().to_owned(),
            message,
        })?;
        let binding = NodeBinding::capture(target.node_data());
        crate::logger::with_node_origin(node, || callback(self.nodes.get_mut(node).expect("live callback"), ctx));
        // Even direct self writes must not duplicate authored effects during activation.
        binding.restore(self.nodes.get_mut(node).expect("live callback").node_data_mut());
        self.restore_authored_node_state_silent(node, state)
    }

    fn publish_authored_node_change(
        &mut self,
        node: NodeId,
        old: Option<ParameterSnapshot>,
        source: Option<NodeId>,
    ) -> Result<(), EngineEditError> {
        self.mark_tree_changed(node);
        self.populate_param_cache_entry(node);
        self.clear_tick_tree_snapshot();
        self.mark_schedule_dirty();
        if let (Some(old), Some(new)) = (old, self.nodes.get(node).and_then(Node::engine_param_snapshot)) {
            self.emit_param_events_for_state_change(node, &old.value, &new.value, source);
            self.emit_param_constraints_event(node, old.constraints, new.constraints);
        }
        let changes = self.subtree_effective_enabled_changes(node);
        let completion = self.queue_effective_enabled_callbacks(&changes);
        if let Some(snapshot) = self.ui_node_dto_for_event(node) {
            let parent = self.nodes.get(node).and_then(|node| node.node_data().parent);
            let index = parent.and_then(|parent| self.ui_child_index(parent, node));
            self.push_ui_graph_transaction(vec![UiGraphOp::NodeCreated {
                snapshot: Box::new(snapshot),
                parent,
                index,
            }]);
        }
        completion
    }
}

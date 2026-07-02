use super::*;

impl<T: Node> Engine<T> {
    #[cfg(debug_assertions)]
    pub(super) fn log_verbose_stabilization_trace(&self, pass: usize, stage: &str) {
        if pass < DEBUG_VERBOSE_STABILIZATION_THRESHOLD {
            return;
        }

        if pass == DEBUG_VERBOSE_STABILIZATION_THRESHOLD {
            eprintln!(
                "[runtime][stabilization] tick={} exceeded {} passes; enabling verbose trace",
                self.time.tick, DEBUG_VERBOSE_STABILIZATION_THRESHOLD
            );
        }

        eprintln!(
            "[runtime][stabilization] tick={} pass={} stage={} pending_edits={} inbox_events={}",
            self.time.tick,
            pass,
            stage,
            self.edits.pending.len(),
            self.inbox.events.len()
        );

        for (index, request) in self
            .edits
            .pending
            .iter()
            .take(DEBUG_VERBOSE_STABILIZATION_SAMPLE_LIMIT)
            .enumerate()
        {
            eprintln!("  edit[{index}] {}", self.describe_pending_edit(request));
        }
        if self.edits.pending.len() > DEBUG_VERBOSE_STABILIZATION_SAMPLE_LIMIT {
            eprintln!(
                "  ... {} more pending edits",
                self.edits.pending.len() - DEBUG_VERBOSE_STABILIZATION_SAMPLE_LIMIT
            );
        }

        for (index, event) in self
            .inbox
            .events
            .iter()
            .take(DEBUG_VERBOSE_STABILIZATION_SAMPLE_LIMIT)
            .enumerate()
        {
            eprintln!("  event[{index}] {}", self.describe_inbox_event(event));
        }
        if self.inbox.events.len() > DEBUG_VERBOSE_STABILIZATION_SAMPLE_LIMIT {
            eprintln!(
                "  ... {} more inbox events",
                self.inbox.events.len() - DEBUG_VERBOSE_STABILIZATION_SAMPLE_LIMIT
            );
        }
    }

    #[cfg(debug_assertions)]
    pub(super) fn describe_pending_edit(&self, request: &EditRequest) -> String {
        match &request.edit {
            Edit::BeginEditSession {
                origin, client_edit_id, ..
            } => format!("BeginEditSession origin={origin:?} client_edit_id={client_edit_id}"),
            Edit::EndEditSession { client_edit_id } => {
                format!("EndEditSession client_edit_id={client_edit_id}")
            }
            Edit::SetParam { node, .. } => format!("SetParam target={}", self.describe_node(*node)),
            Edit::SetParamConstraints { node, .. } => {
                format!("SetParamConstraints target={}", self.describe_node(*node))
            }
            Edit::SetNodeScriptProperty { node, property, .. } => {
                format!(
                    "SetNodeScriptProperty target={} property={property}",
                    self.describe_node(*node)
                )
            }
            Edit::CallNodeScriptMethod { node, method, .. } => {
                format!(
                    "CallNodeScriptMethod target={} method={method}",
                    self.describe_node(*node)
                )
            }
            Edit::CallNodeMutation { node, .. } => {
                format!("CallNodeMutation target={}", self.describe_node(*node))
            }
            Edit::AddNode {
                node,
                parent,
                prev_sibling,
            } => format!(
                "AddNode type='{}' parent={} after={}",
                node.get_type(),
                self.describe_node(*parent),
                self.describe_optional_node(*prev_sibling)
            ),
            Edit::AddNodeTree {
                tree,
                parent,
                prev_sibling,
            } => format!(
                "AddNodeTree root_type='{}' parent={} after={}",
                tree.node_type(),
                self.describe_node(*parent),
                self.describe_optional_node(*prev_sibling)
            ),
            Edit::AddUserItemTree {
                tree,
                parent,
                prev_sibling,
            } => format!(
                "AddUserItemTree root_type='{}' parent={} after={}",
                tree.node_type(),
                self.describe_node(*parent),
                self.describe_optional_node(*prev_sibling)
            ),
            Edit::AddUserItem {
                node,
                parent,
                prev_sibling,
            } => format!(
                "AddUserItem type='{}' parent={} after={}",
                node.get_type(),
                self.describe_node(*parent),
                self.describe_optional_node(*prev_sibling)
            ),
            Edit::CreateBlueprintInstance {
                blueprint_id,
                parent,
                prev_sibling,
                label,
            } => format!(
                "CreateBlueprintInstance blueprint='{blueprint_id}' label={} parent={} after={}",
                label.as_deref().unwrap_or("-"),
                self.describe_node(*parent),
                self.describe_optional_node(*prev_sibling)
            ),
            Edit::ReplaceNode { node, new_node } => format!(
                "ReplaceNode target={} replacement_type='{}'",
                self.describe_node(*node),
                new_node.get_type()
            ),
            Edit::RemoveNode { node } => format!("RemoveNode target={}", self.describe_node(*node)),
            Edit::MoveNode {
                node,
                new_parent,
                new_prev_sibling,
            } => format!(
                "MoveNode node={} new_parent={} after={}",
                self.describe_node(*node),
                self.describe_node(*new_parent),
                self.describe_optional_node(*new_prev_sibling)
            ),
            Edit::PatchMeta { node, .. } => {
                format!("PatchMeta target={}", self.describe_node(*node))
            }
            Edit::SetScriptConfig { node, force_reload, .. } => {
                format!(
                    "SetScriptConfig target={} force_reload={force_reload}",
                    self.describe_node(*node)
                )
            }
            Edit::SetNodeWarning { node, warning } => {
                format!(
                    "SetNodeWarning target={} warning_id='{}'",
                    self.describe_node(*node),
                    warning.id
                )
            }
            Edit::ClearNodeWarning { node, warning_id } => format!(
                "ClearNodeWarning target={} warning_id={}",
                self.describe_node(*node),
                warning_id.as_deref().unwrap_or("*")
            ),
            Edit::SetNodeChildWarningDepth { node, max_depth } => format!(
                "SetNodeChildWarningDepth target={} max_depth={max_depth}",
                self.describe_node(*node)
            ),
            Edit::EmitCustomEvent { event } => format!(
                "EmitCustomEvent topic='{}' origin={}",
                event.topic,
                self.describe_optional_node(event.origin)
            ),
            Edit::ReevaluateGraph => "ReevaluateGraph".to_string(),
            Edit::AddEventListener { subscriber, .. } => {
                format!("AddEventListener subscriber={}", self.describe_node(*subscriber))
            }
            Edit::RemoveEventListener { subscriber, .. } => {
                format!("RemoveEventListener subscriber={}", self.describe_node(*subscriber))
            }
        }
    }

    #[cfg(debug_assertions)]
    pub(super) fn describe_inbox_event(&self, event: &Event) -> String {
        let time = format!("time=({}, {}, {})", event.time.tick, event.time.micro, event.time.seq);
        match &event.kind {
            EventKind::ParamChanged { param, .. } => {
                format!("{time} ParamChanged target={}", self.describe_node(*param))
            }
            EventKind::ParamConstraintsChanged { param, .. } => {
                format!("{time} ParamConstraintsChanged target={}", self.describe_node(*param))
            }
            EventKind::ParamControlChanged { param, .. } => {
                format!("{time} ParamControlChanged target={}", self.describe_node(*param))
            }
            EventKind::ChildAdded { parent, child, decl_id } => format!(
                "{time} ChildAdded parent={} child={} decl='{}'",
                self.describe_node(*parent),
                self.describe_node(*child),
                decl_id.0
            ),
            EventKind::ChildRemoved { parent, child } => format!(
                "{time} ChildRemoved parent={} child={}",
                self.describe_node(*parent),
                self.describe_node(*child)
            ),
            EventKind::ChildReplaced {
                parent,
                old,
                new,
                decl_id,
            } => format!(
                "{time} ChildReplaced parent={} old={} new={} decl='{}'",
                self.describe_node(*parent),
                self.describe_node(*old),
                self.describe_node(*new),
                decl_id.0
            ),
            EventKind::ChildMoved {
                child,
                old_parent,
                new_parent,
            } => format!(
                "{time} ChildMoved child={} old_parent={} new_parent={}",
                self.describe_node(*child),
                self.describe_node(*old_parent),
                self.describe_node(*new_parent)
            ),
            EventKind::ChildReordered { parent, child } => format!(
                "{time} ChildReordered parent={} child={}",
                self.describe_node(*parent),
                self.describe_node(*child)
            ),
            EventKind::NodeCreated { node } => {
                format!("{time} NodeCreated node={}", self.describe_node(*node))
            }
            EventKind::NodeDeleted { node } => format!("{time} NodeDeleted node={node:?}"),
            EventKind::MetaChanged { node, .. } => {
                format!("{time} MetaChanged target={}", self.describe_node(*node))
            }
            EventKind::GraphTransaction { transaction } => {
                format!(
                    "{time} GraphTransaction tx_id={} ops={}",
                    transaction.tx_id,
                    transaction.ops.len()
                )
            }
            EventKind::Custom(event) => format!(
                "{time} Custom topic='{}' origin={}",
                event.topic,
                self.describe_optional_node(event.origin)
            ),
        }
    }

    #[cfg(debug_assertions)]
    pub(super) fn describe_optional_node(&self, node: Option<NodeId>) -> String {
        node.map(|node| self.describe_node(node))
            .unwrap_or_else(|| "None".to_string())
    }

    #[cfg(debug_assertions)]
    pub(super) fn describe_node(&self, node_id: NodeId) -> String {
        let Some(node) = self.nodes.get(node_id) else {
            return format!("{node_id:?} <missing>");
        };
        let data = node.node_data();
        format!(
            "{node_id:?} type='{}' decl='{}' label='{}'",
            node.get_type(),
            data.meta.decl_id.0,
            data.meta.label
        )
    }
}

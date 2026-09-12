use std::{sync::Arc, time::Duration};

use serde_json::Value as JsonValue;

use crate::{
    events::{CustomEvent, Event, EventKind},
    logger,
    node::NodeId,
    parameter::ParamValue,
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};

use super::{
    ManagedLoadChild, ScriptEvent, ScriptHostBridge, ScriptLogLevel, ScriptTreeNodeView, ScriptTreeView,
    managed_child_from_script_call,
};

fn script_log_level_to_logger(level: ScriptLogLevel) -> logger::LogLevel {
    match level {
        ScriptLogLevel::Info => logger::LogLevel::Info,
        ScriptLogLevel::Success => logger::LogLevel::Success,
        ScriptLogLevel::Warning => logger::LogLevel::Warning,
        ScriptLogLevel::Error => logger::LogLevel::Error,
    }
}

impl From<&Event> for ScriptEvent {
    fn from(event: &Event) -> Self {
        let (kind, origin, old_value) = match &event.kind {
            EventKind::ParamChanged { param, old_value, .. } => {
                ("paramChanged".to_string(), Some(*param), Some(old_value.clone()))
            }
            EventKind::ParamConstraintsChanged { param, .. } => {
                ("paramConstraintsChanged".to_string(), Some(*param), None)
            }
            EventKind::ParamControlChanged { param, .. } => ("paramControlChanged".to_string(), Some(*param), None),
            EventKind::ChildAdded { child, .. } => ("childAdded".to_string(), Some(*child), None),
            EventKind::ChildRemoved { parent, .. } => ("childRemoved".to_string(), Some(*parent), None),
            EventKind::ChildReplaced { new, .. } => ("childReplaced".to_string(), Some(*new), None),
            EventKind::ChildMoved { child, .. } => ("childMoved".to_string(), Some(*child), None),
            EventKind::ChildReordered { child, .. } => ("childReordered".to_string(), Some(*child), None),
            EventKind::NodeCreated { node } => ("nodeCreated".to_string(), Some(*node), None),
            EventKind::NodeDeleted { .. } => ("nodeDeleted".to_string(), None, None),
            EventKind::MetaChanged { node, .. } => ("metaChanged".to_string(), Some(*node), None),
            EventKind::GraphTransaction { .. } => ("graphTransaction".to_string(), None, None),
            EventKind::Custom(custom) => ("custom".to_string(), custom.origin, None),
        };
        let payload = serde_json::to_value(&event.kind).unwrap_or(JsonValue::Null);
        Self {
            kind,
            origin,
            old_value,
            payload,
        }
    }
}

impl ScriptTreeView for ProcessTreeSnapshot {
    fn root(&self) -> NodeId {
        ProcessTreeSnapshot::root(self)
    }

    fn node(&self, node: NodeId) -> Option<ScriptTreeNodeView> {
        let snapshot = ProcessTreeSnapshot::node(self, node)?;
        Some(ScriptTreeNodeView {
            id: snapshot.id,
            node_type: snapshot.node_type.clone(),
            decl_id: snapshot.decl_id.clone(),
            short_name: snapshot.short_name.clone(),
            label: snapshot.label.clone(),
            enabled: snapshot.enabled,
            child_count: snapshot.child_count,
            param_value: snapshot.param_value.clone(),
            param_constraints: snapshot.param_constraints.clone(),
        })
    }

    fn find_child(&self, parent: NodeId, key: &str) -> Option<NodeId> {
        ProcessTreeSnapshot::find_child(self, parent, key)
    }

    fn child_ids(&self, parent: NodeId) -> Vec<NodeId> {
        ProcessTreeSnapshot::child_ids(self, parent)
    }

    fn child_at(&self, parent: NodeId, index: usize) -> Option<NodeId> {
        ProcessTreeSnapshot::child_at(self, parent, index)
    }

    fn script_property(&self, node: NodeId, key: &str) -> Option<ParamValue> {
        ProcessTreeSnapshot::node(self, node)?
            .script_property(key)
            .map(|value| value.into_owned())
    }

    fn script_properties(&self, node: NodeId) -> Vec<(String, ParamValue)> {
        ProcessTreeSnapshot::node(self, node)
            .map(|snapshot| {
                snapshot
                    .script_properties
                    .iter()
                    .map(|(key, value)| (key.clone(), value.clone()))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn has_script_method(&self, node: NodeId, method: &str) -> bool {
        ProcessTreeSnapshot::node(self, node).is_some_and(|snapshot| snapshot.has_script_method(method))
    }

    fn describe_node(&self, node: NodeId) -> Option<String> {
        ProcessTreeSnapshot::node(self, node).map(ToString::to_string)
    }
}

pub(super) struct NodeScriptHostBridge<'a> {
    script_node: NodeId,
    host_node: Option<NodeId>,
    started_elapsed: Duration,
    runtime_subscriptions: &'a mut Vec<crate::node::EventSubscription>,
    load_declared_children: Option<&'a mut Vec<ManagedLoadChild>>,
    ctx: &'a mut ProcessCtx,
}

impl<'a> NodeScriptHostBridge<'a> {
    pub(super) fn new(
        script_node: NodeId,
        host_node: Option<NodeId>,
        started_elapsed: Duration,
        runtime_subscriptions: &'a mut Vec<crate::node::EventSubscription>,
        load_declared_children: Option<&'a mut Vec<ManagedLoadChild>>,
        ctx: &'a mut ProcessCtx,
    ) -> Self {
        Self {
            script_node,
            host_node,
            started_elapsed,
            runtime_subscriptions,
            load_declared_children,
            ctx,
        }
    }
}

impl ScriptHostBridge for NodeScriptHostBridge<'_> {
    fn owner_node(&self) -> Option<NodeId> {
        self.host_node
    }

    fn script_node(&self) -> Option<NodeId> {
        Some(self.script_node)
    }

    fn time_seconds(&self) -> f64 {
        self.ctx
            .runtime_elapsed
            .saturating_sub(self.started_elapsed)
            .as_secs_f64()
    }

    fn delta_seconds(&self) -> f64 {
        self.ctx.delta_time.as_secs_f64()
    }

    fn log(&mut self, level: ScriptLogLevel, message: &str) {
        let _ = logger::log_message(
            script_log_level_to_logger(level),
            "script".to_string(),
            Some(self.script_node),
            message.to_string(),
        );
    }

    fn emit_custom(&mut self, topic: &str, payload: JsonValue) -> Result<(), String> {
        self.ctx
            .emit_custom_event(CustomEvent::new(topic, Some(self.script_node), payload));
        Ok(())
    }

    fn tree_snapshot(&self) -> Option<Arc<dyn ScriptTreeView>> {
        self.ctx
            .tree_snapshot_arc()
            .map(|snapshot| snapshot as Arc<dyn ScriptTreeView>)
    }

    fn set_node_script_property(&mut self, node: NodeId, property: String, value: ParamValue) -> Result<(), String> {
        self.ctx.set_node_script_property(node, property, value);
        Ok(())
    }

    fn call_node_script_method(&mut self, node: NodeId, method: String, args: Vec<ParamValue>) -> Result<(), String> {
        if let Some(load_declared_children) = self.load_declared_children.as_deref_mut()
            && let Some(managed_child) = managed_child_from_script_call(node, method.as_str(), args.as_slice())
            && !load_declared_children.contains(&managed_child)
        {
            load_declared_children.push(managed_child);
        }

        self.ctx.call_node_script_method(node, method, args);
        Ok(())
    }

    fn set_event_listener(&mut self, target: NodeId, level: u32) -> Result<(), String> {
        let previous_levels = self
            .runtime_subscriptions
            .iter()
            .filter(|entry| entry.node == target)
            .map(|entry| entry.max_depth)
            .collect::<Vec<_>>();

        if previous_levels.len() == 1 && previous_levels[0] == level {
            return Ok(());
        }

        for previous in previous_levels {
            self.ctx
                .remove_event_listener_subtree(self.script_node, target, previous);
        }
        self.runtime_subscriptions.retain(|entry| entry.node != target);

        self.ctx.add_event_listener_subtree(self.script_node, target, level);
        self.runtime_subscriptions
            .push(crate::node::EventSubscription::subtree(target, level));
        Ok(())
    }

    fn remove_event_listener(&mut self, target: NodeId) -> Result<(), String> {
        let previous_levels = self
            .runtime_subscriptions
            .iter()
            .filter(|entry| entry.node == target)
            .map(|entry| entry.max_depth)
            .collect::<Vec<_>>();

        for previous in previous_levels {
            self.ctx
                .remove_event_listener_subtree(self.script_node, target, previous);
        }
        self.runtime_subscriptions.retain(|entry| entry.node != target);
        Ok(())
    }

    fn clear_event_listeners(&mut self) -> Result<(), String> {
        let script_node = self.script_node;
        for subscription in self.runtime_subscriptions.drain(..) {
            self.ctx
                .remove_event_listener_subtree(script_node, subscription.node, subscription.max_depth);
        }
        Ok(())
    }
}

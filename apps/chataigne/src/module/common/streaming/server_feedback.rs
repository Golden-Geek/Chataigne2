use std::collections::BTreeMap;

use golden_core::{
    edit::{Edit, EditCompletion, EditCompletionStatus},
    node::{NodeHandle, NodeId},
    parameter::{ParamValue, Parameter},
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};

#[derive(Default)]
pub(crate) struct ServerFeedback {
    connected: bool,
    warning: Option<String>,
    completion: Option<EditCompletion>,
}

pub(crate) struct ServerFeedbackTargets {
    pub connected: Option<NodeId>,
    pub port: NodeId,
    pub count: NodeId,
    pub clients: Option<NodeId>,
    pub warning_id: &'static str,
}

impl ServerFeedback {
    pub fn is_pending(&self) -> bool {
        self.completion.is_some()
    }
    pub fn failed(&self) -> bool {
        self.completion
            .as_ref()
            .is_some_and(|receipt| receipt.status() == EditCompletionStatus::Failed)
    }
    pub fn set_connected(&mut self, connected: bool) -> bool {
        if self.connected == connected {
            return false;
        }
        self.connected = connected;
        true
    }
    pub fn set_warning(&mut self, warning: Option<&str>) -> bool {
        if self.warning.as_deref() == warning {
            return false;
        }
        self.warning = warning.map(str::to_owned);
        true
    }
    pub fn settle(&mut self, dirty: &mut bool) {
        if let Some(receipt) = self.completion.take() {
            match receipt.status() {
                EditCompletionStatus::Applied => {}
                EditCompletionStatus::Failed => *dirty = true,
                EditCompletionStatus::Pending => self.completion = Some(receipt),
            }
        }
    }
    /// The caller wakes service when a batch is staged. Reconcile a failed prefix
    /// against the next snapshot instead of replaying structural edits.
    pub fn reconcile(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        dirty: &mut bool,
        targets: ServerFeedbackTargets,
        clients: &BTreeMap<String, String>,
        create_client: fn(&str, &str) -> Parameter,
    ) -> bool {
        self.settle(dirty);
        if self.is_pending() || !*dirty {
            return false;
        }
        *dirty = false;
        // Only this desired-state projection owns the receipt, separate from
        // native callbacks and graph input already queued in the same context.
        let previous = std::mem::take(&mut ctx.edits);
        self.write_status(ctx, snapshot, &targets);
        Self::write_clients(ctx, snapshot, &targets, clients, create_client);
        let mut projection = std::mem::replace(&mut ctx.edits, previous);
        if projection.pending.is_empty() {
            return false;
        }
        self.completion = Some(projection.completion());
        ctx.edits.pending.extend(projection.drain());
        true
    }
    fn write_status(&self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot, targets: &ServerFeedbackTargets) {
        if let Some(connected) = targets.connected {
            if snapshot.node(connected).and_then(|node| node.param_value.as_ref())
                != Some(&ParamValue::Bool(self.connected))
            {
                ctx.set_param(connected, ParamValue::Bool(self.connected));
            }
        }
        if let Some(port) = snapshot.node(targets.port) {
            let current = port.presentation.warning(Some(targets.warning_id));
            if current.map(|warning| warning.message.as_str()) != self.warning.as_deref() {
                let handle = NodeHandle::new(targets.port);
                match &self.warning {
                    Some(message) => handle.set_warning_with(ctx, Some(targets.warning_id), message.as_str(), None),
                    None => handle.clear_warning(ctx, Some(targets.warning_id)),
                }
            }
        }
    }
    fn write_clients(
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        targets: &ServerFeedbackTargets,
        clients: &BTreeMap<String, String>,
        create_client: fn(&str, &str) -> Parameter,
    ) {
        let count = i32::try_from(clients.len()).unwrap_or(i32::MAX);
        if let Some(node) = snapshot.node(targets.count) {
            if node.param_value.as_ref() != Some(&ParamValue::Int(count)) {
                ctx.set_param(targets.count, ParamValue::Int(count));
            }
        }
        let Some(clients_id) = targets.clients else { return };
        for (client_id, info) in clients {
            match snapshot.find_child(clients_id, client_id) {
                Some(existing_id) => {
                    let Some(existing) = snapshot.node(existing_id) else {
                        continue;
                    };
                    match existing.param_value.as_ref() {
                        Some(ParamValue::Str(current)) if current == info => {}
                        Some(ParamValue::Str(_)) => ctx.set_param(existing_id, ParamValue::Str(info.clone())),
                        _ => ctx.replace_node_boxed(existing_id, Box::new(create_client(client_id, info))),
                    }
                }
                None => ctx.add_child_boxed(clients_id, Box::new(create_client(client_id, info)), None),
            }
        }
        for child in snapshot.child_ids(clients_id) {
            if let Some(node) = snapshot.node(child) {
                if !clients.contains_key(&node.label) {
                    ctx.edits.push(Edit::RemoveNode { node: child });
                }
            }
        }
    }
}

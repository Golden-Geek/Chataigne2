use super::{create_client_info_param, WebSocketServerModule, WEBSOCKET_SERVER_PORT_WARNING_ID};
use golden_core::{
    edit::{Edit, EditCompletion, EditCompletionStatus},
    node::NodeHandle,
    parameter::ParamValue,
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};

#[derive(Default)]
pub(super) struct ServerFeedback {
    connected: bool,
    warning: Option<String>,
    completion: Option<EditCompletion>,
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
}
impl WebSocketServerModule {
    pub(super) fn set_connected(&mut self, connected: bool) {
        if self.feedback.connected != connected {
            self.feedback.connected = connected;
            self.client_list_dirty = true;
        }
    }
    pub(super) fn set_port_warning(&mut self, message: &str) {
        if self.feedback.warning.as_deref() != Some(message) {
            self.feedback.warning = Some(message.to_owned());
            self.client_list_dirty = true;
        }
    }
    pub(super) fn clear_port_warning(&mut self) {
        if self.feedback.warning.take().is_some() {
            self.client_list_dirty = true;
        }
    }
    pub(super) fn settle_feedback(&mut self) {
        if let Some(receipt) = self.feedback.completion.take() {
            match receipt.status() {
                EditCompletionStatus::Applied => {}
                EditCompletionStatus::Failed => self.client_list_dirty = true,
                EditCompletionStatus::Pending => self.feedback.completion = Some(receipt),
            }
        }
    }
    pub(super) fn sync_client_nodes(&mut self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot) {
        self.settle_feedback();
        if self.feedback.is_pending() || !self.client_list_dirty {
            return;
        }
        self.client_list_dirty = false;
        // Observe only this desired-state projection, separately from callbacks and graph input.
        // A failed prefix is reconciled against the next snapshot; never replay structural edits.
        let previous = std::mem::take(&mut ctx.edits);
        self.write_feedback(ctx, snapshot);
        self.write_client_nodes(ctx, snapshot);
        let mut projection = std::mem::replace(&mut ctx.edits, previous);
        if !projection.pending.is_empty() {
            self.feedback.completion = Some(projection.completion());
            ctx.edits.pending.extend(projection.drain());
            self.request_service();
        }
    }
    fn write_feedback(&self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot) {
        if let Some(connection) = self.stream.connection_id() {
            if let Some(connected) = snapshot.find_child_by_decl_id(connection, "connected") {
                if snapshot.node(connected).and_then(|node| node.param_value.as_ref())
                    != Some(&ParamValue::Bool(self.feedback.connected))
                {
                    ctx.set_param(connected, ParamValue::Bool(self.feedback.connected));
                }
            }
        }
        if let Some(port) = snapshot.node(self.port.id()) {
            let current = port.presentation.warning(Some(WEBSOCKET_SERVER_PORT_WARNING_ID));
            if current.map(|warning| warning.message.as_str()) != self.feedback.warning.as_deref() {
                let handle = NodeHandle::new(self.port.id());
                match &self.feedback.warning {
                    Some(message) => {
                        handle.set_warning_with(ctx, Some(WEBSOCKET_SERVER_PORT_WARNING_ID), message.as_str(), None)
                    }
                    None => handle.clear_warning(ctx, Some(WEBSOCKET_SERVER_PORT_WARNING_ID)),
                }
            }
        }
    }
    fn write_client_nodes(&mut self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot) {
        let connected_clients = i32::try_from(self.client_infos.len()).unwrap_or(i32::MAX);
        if self.connected_clients.is_bound()
            && snapshot
                .node(self.connected_clients.id())
                .and_then(|node| node.param_value.as_ref())
                != Some(&ParamValue::Int(connected_clients))
        {
            ctx.set_param(self.connected_clients.id(), ParamValue::Int(connected_clients));
        }

        let Some(clients_id) = self.clients_node_id(snapshot) else {
            return;
        };

        for (client_id, info) in &self.client_infos {
            match snapshot.find_child(clients_id, client_id.as_str()) {
                Some(existing_id) => {
                    let Some(existing_snapshot) = snapshot.node(existing_id) else {
                        continue;
                    };

                    match existing_snapshot.param_value.as_ref() {
                        Some(ParamValue::Str(existing_info)) if existing_info == info => {}
                        Some(ParamValue::Str(_)) => {
                            ctx.set_param(existing_id, ParamValue::Str(info.clone()));
                        }
                        _ => {
                            ctx.replace_node_boxed(existing_id, Box::new(create_client_info_param(client_id, info)));
                        }
                    }
                }
                None => {
                    ctx.add_child_boxed(clients_id, Box::new(create_client_info_param(client_id, info)), None);
                }
            }
        }

        for child_id in snapshot.child_ids(clients_id) {
            let Some(child_snapshot) = snapshot.node(child_id) else {
                continue;
            };

            if !self.client_infos.contains_key(child_snapshot.label.as_str()) {
                ctx.edits.push(Edit::RemoveNode { node: child_id });
            }
        }
    }
}

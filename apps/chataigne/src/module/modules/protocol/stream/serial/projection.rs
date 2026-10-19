use super::{SerialModule, SERIAL_PORT_CONNECTION_WARNING_ID, SERIAL_PORT_OPTIONS_WARNING_ID};
use crate::app::module::common::serial::{sync_serial_port_enum_options, SerialDiscoverySnapshot};
use golden_core::{
    edit::{EditCompletion, EditCompletionStatus},
    node::NodeHandle,
    parameter::ParamValue,
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};

#[derive(Default)]
pub(super) struct SerialProjection {
    pub catalog: SerialDiscoverySnapshot,
    connected: bool,
    warning: Option<String>,
    registration_error: Option<String>,
    dirty: bool,
    selection: u64,
    applied_catalog: Option<(u64, u64)>,
    completion: Option<(EditCompletion, (u64, u64))>,
}
impl SerialProjection {
    pub fn needs_snapshot(&self) -> bool {
        (!self.pending() && self.dirty)
            || self
                .completion
                .as_ref()
                .is_some_and(|(receipt, _)| receipt.status() == EditCompletionStatus::Failed)
    }
    pub fn pending(&self) -> bool {
        self.completion.is_some()
    }
    pub fn settle(&mut self) {
        if let Some((receipt, ticket)) = self.completion.take() {
            match receipt.status() {
                EditCompletionStatus::Applied => self.applied_catalog = Some(ticket),
                EditCompletionStatus::Failed => self.dirty = true,
                EditCompletionStatus::Pending => self.completion = Some((receipt, ticket)),
            }
        }
    }
    pub fn connected(&mut self, connected: bool) {
        self.dirty |= self.connected != connected;
        self.connected = connected;
    }
    pub fn warning(&mut self, warning: Option<&str>) {
        self.dirty |= self.warning.as_deref() != warning;
        self.warning = warning.map(str::to_owned);
    }
    pub fn registration_error(&mut self, error: Option<String>) {
        self.dirty |= self.registration_error != error;
        self.registration_error = error;
    }
    pub fn catalog(&mut self, catalog: SerialDiscoverySnapshot) {
        self.dirty |= self.catalog.version != catalog.version || self.applied_catalog.is_none();
        self.catalog = catalog;
    }
    pub fn selection_changed(&mut self) {
        self.selection = self
            .selection
            .checked_add(1)
            .expect("serial selection generation exhausted");
        self.dirty = true;
    }
}
impl SerialModule {
    pub(super) fn project_feedback(&mut self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot) {
        self.projection.settle();
        if self.projection.pending() || !self.projection.needs_snapshot() || !self.port_name.is_bound() {
            return;
        }
        self.projection.dirty = false;
        let previous = std::mem::take(&mut ctx.edits);
        let ticket = (self.projection.catalog.version, self.projection.selection);
        if self.projection.applied_catalog != Some(ticket) {
            sync_serial_port_enum_options(ctx, self.port_name.id(), self.projection.catalog.catalog.clone());
        }
        if let Some(connected) = self
            .stream
            .connection_id()
            .and_then(|id| snapshot.find_child_by_decl_id(id, "connected"))
        {
            if snapshot.node(connected).and_then(|node| node.param_value.as_ref())
                != Some(&ParamValue::Bool(self.projection.connected))
            {
                ctx.set_param(connected, ParamValue::Bool(self.projection.connected));
            }
        }
        let options_error = self
            .projection
            .registration_error
            .as_deref()
            .or(self.projection.catalog.error.as_deref());
        if let Some(port) = snapshot.node(self.port_name.id()) {
            for (id, desired) in [
                (SERIAL_PORT_CONNECTION_WARNING_ID, self.projection.warning.as_deref()),
                (SERIAL_PORT_OPTIONS_WARNING_ID, options_error),
            ] {
                if port
                    .presentation
                    .warning(Some(id))
                    .map(|warning| warning.message.as_str())
                    != desired
                {
                    let handle = NodeHandle::new(self.port_name.id());
                    match desired {
                        Some(message) => handle.set_warning_with(ctx, Some(id), message, None),
                        None => handle.clear_warning(ctx, Some(id)),
                    }
                }
            }
        }
        let mut projection = std::mem::replace(&mut ctx.edits, previous);
        if projection.pending.is_empty() {
            self.projection.applied_catalog = Some(ticket);
        } else {
            self.projection.completion = Some((projection.completion(), ticket));
            ctx.edits.pending.extend(projection.drain());
            self.request_service();
        }
    }
}

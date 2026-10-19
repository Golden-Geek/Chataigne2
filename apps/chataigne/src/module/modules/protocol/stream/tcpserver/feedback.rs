use super::{create_client_info_param, TcpServerModule, TCP_SERVER_PORT_WARNING_ID};
use crate::app::module::common::streaming::server_feedback::ServerFeedbackTargets;
use golden_core::process_ctx::{ProcessCtx, ProcessTreeSnapshot};

impl TcpServerModule {
    pub(super) fn set_connected(&mut self, connected: bool) {
        self.client_list_dirty |= self.feedback.set_connected(connected);
    }
    pub(super) fn set_port_warning(&mut self, message: &str) {
        self.client_list_dirty |= self.feedback.set_warning(Some(message));
    }
    pub(super) fn clear_port_warning(&mut self) {
        self.client_list_dirty |= self.feedback.set_warning(None);
    }
    pub(super) fn settle_feedback(&mut self) {
        self.feedback.settle(&mut self.client_list_dirty);
    }
    pub(super) fn sync_client_nodes(&mut self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot) {
        let targets = ServerFeedbackTargets {
            connected: self
                .stream
                .connection_id()
                .and_then(|connection| snapshot.find_child_by_decl_id(connection, "connected")),
            port: self.port.id(),
            count: self.connected_clients.id(),
            clients: self.clients_node_id(snapshot),
            warning_id: TCP_SERVER_PORT_WARNING_ID,
        };
        if self.feedback.reconcile(
            ctx,
            snapshot,
            &mut self.client_list_dirty,
            targets,
            &self.client_infos,
            create_client_info_param,
        ) {
            self.request_service();
        }
    }
}

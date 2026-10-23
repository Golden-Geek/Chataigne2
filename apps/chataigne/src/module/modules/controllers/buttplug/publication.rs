use super::*;
use golden_core::edit::{Edit, EditPreconditions, NodeReadGuard};
use golden_io::RetainedPayload;
use transport::Publication;

impl ButtplugModule {
    pub(super) fn devices(&self) -> &[ButtplugDeviceInfo] {
        self.known_devices
            .as_ref()
            .map_or(&[], |publication| publication.devices())
    }
    pub(super) fn project_event(&mut self, ctx: &mut ProcessCtx, event: RetainedPayload<Publication>) {
        if !event.current() {
            self.set_target_warning(ctx, "Buttplug input source/session retired; publication was cancelled");
            return;
        }
        let Some(snapshot) = ctx.tree_snapshot_arc() else {
            return;
        };
        let mut reads = Vec::new();
        let mut targets = vec![self.id()];
        for key in [
            "connection/connected",
            "connection/remote_host",
            "connection/device",
            "values/info/server_name",
            "values/info/connected_devices",
            "values/info/scanning",
            "values/info/last_event",
        ] {
            let mut parent = self.id();
            for key in key.split('/') {
                let Some(child) = snapshot.find_child_by_decl_id(parent, key) else {
                    self.set_target_warning(ctx, "Buttplug publication target is unavailable");
                    return;
                };
                parent = child;
                targets.push(child);
            }
            targets.push(parent);
        }
        targets.sort_unstable_by_key(|target| target.0);
        targets.dedup();
        for target in targets {
            let guard = if target == self.id() {
                NodeReadGuard::capture_identity(&snapshot, target)
            } else {
                NodeReadGuard::capture_structure(&snapshot, target)
            };
            let Some(guard) = guard else {
                return;
            };
            reads.push(guard);
        }
        ctx.edits.push(Edit::CheckPreconditions {
            preconditions: EditPreconditions::new(reads, event.guards.to_vec()).unwrap(),
        });
        if let Some(catalog) = &event.catalog {
            if self.device.is_bound() {
                if let Some(node) = snapshot.node(self.device.id()) {
                    ctx.edits.push(Edit::SetParamEnumOptions {
                        node: node.id,
                        expected_uuid: node.uuid,
                        expected_value: ParamValue::Enum(catalog.selection.clone()),
                        options: catalog.options.clone(),
                    });
                }
            }
        }
        match &event.event {
            ButtplugWorkerEvent::Status(ButtplugConnectionStatus::Connected { server_name }) => {
                golden_core::logsuccess!(origin=self.id(); format!("Connected Buttplug server '{server_name}'."));
                self.clear_target_warning(ctx);
                self.base.set_connected(ctx, true);
                self.set_string_param(ctx, ButtplugStringParam::ServerName, server_name);
                self.set_last_event(ctx, format!("Connected to {server_name}"));
            }
            ButtplugWorkerEvent::Status(ButtplugConnectionStatus::Recovering { message }) => {
                logerror!(format!("Buttplug transport recovering: {message}"));
                self.set_target_warning(ctx, message);
                self.base.set_connected(ctx, false);
                self.set_bool_param(ctx, ButtplugBoolParam::Scanning, false);
                self.known_devices = Some(event.clone());
                self.refresh_device_values(ctx);
                self.set_last_event(ctx, format!("Recovering: {message}"));
            }
            ButtplugWorkerEvent::Devices(_) => {
                self.known_devices = Some(event.clone());
                self.refresh_device_values(ctx);
                self.refresh_selection_warning(ctx);
            }
            ButtplugWorkerEvent::DeviceAdded(device) => {
                self.set_last_event(ctx, format!("Device added: {}", device_label(device)))
            }
            ButtplugWorkerEvent::DeviceRemoved(device) => {
                self.set_last_event(ctx, format!("Device removed: {}", device_label(device)))
            }
            ButtplugWorkerEvent::Scanning(scanning) => self.set_bool_param(ctx, ButtplugBoolParam::Scanning, *scanning),
            ButtplugWorkerEvent::ScanningFinished => self.set_last_event(ctx, "Scanning finished".into()),
            ButtplugWorkerEvent::Error(error) => {
                self.set_target_warning(ctx, error);
                logerror!(error);
            }
        }
        if let Some(callback) = &event.callback {
            ctx.emit_custom_event(callback.event(self.id()));
            self.base.emit_incoming_traffic(ctx);
            if self.base.log_incoming_enabled() {
                golden_core::log!(origin=self.id(); "Buttplug input received");
            }
        }
        ctx.edits.retain_payload(event);
    }
}

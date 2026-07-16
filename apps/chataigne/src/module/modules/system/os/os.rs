mod os_runtime;
#[cfg(test)]
mod os_tests;

use golden_core::{
    edit::Edit,
    engine::NodeExecutionRule,
    events::CustomEvent,
    logerror, node,
    node::{Node, NodeId, NodeScriptDescriptor},
    parameter::ParamValue,
    process_ctx::ProcessCtx,
};

use crate::app::module::common::os::{
    OsControlRequest, WakeOnLanRequest, OS_LOGOUT_COMMAND_NODE_TYPE,
    OS_MODULE_COMMAND_TYPES, OS_REBOOT_COMMAND_NODE_TYPE, OS_SHUTDOWN_COMMAND_NODE_TYPE,
    OS_WAKE_ON_LAN_COMMAND_NODE_TYPE,
};
use crate::app::module::common::system_metrics;

use self::os_runtime::{HostControlAction, OsMetricsSnapshot, OsRuntime};

const OS_MODULE_UPDATE_RATE_HZ: u32 = 1;
const OS_SCRIPT_METHODS: &[&str] = &["shutdown", "reboot", "logout", "wakeOnLan"];
const SYSTEM_STATS_UPDATED_CALLBACK: &str = "systemStatsUpdated";
const SYSTEM_COMMAND_REQUESTED_CALLBACK: &str = "systemCommandRequested";
const SYSTEM_COMMAND_FAILED_CALLBACK: &str = "systemCommandFailed";

#[node("os_module", label = "OS")]
#[children(
    folder(connection) {
        [base_children];
    }
    folder(parameters) {
        [base_children];
    }
    folder(values) {
        folder(info, label = "Info", collapsed = true) {
            os_type: String = String::new() (
                label = "OS Type",
                description = "Operating system family of the host running this app.",
                read_only = true
            );
            architecture: String = String::new() (
                label = "Architecture",
                description = "CPU architecture reported by the current host.",
                read_only = true
            );
            os_version: String = String::new() (
                label = "OS Version",
                description = "Host operating system version.",
                read_only = true
            );
        }
        folder(cpu, label = "CPU") {
            global_ratio: f64 = 0.0 (
                label = "Global Usage",
                description = "Global CPU usage ratio across the host, normalized to 0.0-1.0.",
                read_only = true,
                min = 0.0,
                max = 1.0,
                widget = "text"
            );
            app_ratio: f64 = 0.0 (
                label = "App Usage",
                description = "CPU usage ratio attributed to this app process, normalized to 0.0-1.0 of total host capacity.",
                read_only = true,
                min = 0.0,
                max = 1.0,
                widget = "text"
            );
        }
        folder(memory, label = "Memory") {
            system_used_mb: f64 = 0.0 (
                label = "System Used MB",
                description = "System memory currently in use, expressed in megabytes.",
                read_only = true,
                min = 0.0,
                widget = "text"
            );
            system_total_mb: f64 = 0.0 (
                label = "System Total MB",
                description = "Total system memory available on the host, expressed in megabytes.",
                read_only = true,
                min = 0.0,
                widget = "text"
            );
            app_used_mb: f64 = 0.0 (
                label = "App Used MB",
                description = "Resident memory used by this app process, expressed in megabytes.",
                read_only = true,
                min = 0.0,
                widget = "text"
            );
            app_virtual_mb: f64 = 0.0 (
                label = "App Virtual MB",
                description = "Virtual memory used by this app process, expressed in megabytes.",
                read_only = true,
                min = 0.0,
                widget = "text"
            );
        }
        folder(network, label = "Network") {
            received_mb_per_sec: f64 = 0.0 (
                label = "Received MB Per Sec",
                description = "Megabytes received across host network interfaces since the last refresh.",
                read_only = true,
                min = 0.0,
                widget = "text"
            );
            transmitted_mb_per_sec: f64 = 0.0 (
                label = "Transmitted MB Per Sec",
                description = "Megabytes transmitted across host network interfaces since the last refresh.",
                read_only = true,
                min = 0.0,
                widget = "text"
            );
            total_received_mb: f64 = 0.0 (
                label = "Total Received MB",
                description = "Total megabytes received across host network interfaces since boot.",
                read_only = true,
                min = 0.0,
                widget = "text"
            );
            total_transmitted_mb: f64 = 0.0 (
                label = "Total Transmitted MB",
                description = "Total megabytes transmitted across host network interfaces since boot.",
                read_only = true,
                min = 0.0,
                widget = "text"
            );
        }
        folder(uptime, label = "Uptime") {
            system_seconds: f64 = 0.0 (
                label = "System Uptime",
                description = "Time since the host booted.",
                read_only = true,
                min = 0.0,
                widget = "time"
            );
            app_seconds: f64 = 0.0 (
                label = "App Uptime",
                description = "Time since this app process started.",
                read_only = true,
                min = 0.0,
                widget = "time"
            );
        }
        [base_children];
    }
)]
pub struct OsModule {
    base: crate::app::ModuleBase,
    runtime: OsRuntime,
}

impl OsModule {
    pub fn create() -> Self {
        Self::new(crate::app::ModuleBase::new(), OsRuntime::create())
    }

    fn refresh_static_values(&mut self, ctx: &mut ProcessCtx) {
        self.os_type.set(ctx, os_runtime::host_os_type().to_string());
        self.architecture.set(ctx, os_runtime::host_architecture());
        self.os_version.set(ctx, os_runtime::current_os_version());
    }

    fn refresh_metrics(&mut self, ctx: &mut ProcessCtx) {
        let snapshot = self.runtime.snapshot();
        self.sync_metric_constraints(ctx, &snapshot);
        self.apply_metrics_snapshot(ctx, &snapshot);
        self.base.emit_incoming_traffic(ctx);

        if self.base.log_incoming_enabled() {
            golden_core::log!(origin = self.id(); format!(
                "System stats refreshed: cpu {:.3} (app {:.3}), memory {:.1}/{:.1} MB, network down {:.3} MB/s up {:.3} MB/s.",
                snapshot.global_cpu_ratio,
                snapshot.app_cpu_ratio,
                snapshot.system_used_mb,
                snapshot.system_total_mb,
                snapshot.received_mb_per_sec,
                snapshot.transmitted_mb_per_sec,
            ));
        }

        self.emit_system_stats_callback(ctx, &snapshot);
    }

    fn apply_metrics_snapshot(&mut self, ctx: &mut ProcessCtx, snapshot: &OsMetricsSnapshot) {
        self.os_version.set(ctx, snapshot.os_version.clone());
        self.global_ratio.set(ctx, snapshot.global_cpu_ratio);
        self.app_ratio.set(ctx, snapshot.app_cpu_ratio);
        self.system_used_mb.set(ctx, snapshot.system_used_mb);
        self.system_total_mb.set(ctx, snapshot.system_total_mb);
        self.app_used_mb.set(ctx, snapshot.app_used_mb);
        self.app_virtual_mb.set(ctx, snapshot.app_virtual_mb);
        self.received_mb_per_sec.set(ctx, snapshot.received_mb_per_sec);
        self.transmitted_mb_per_sec
            .set(ctx, snapshot.transmitted_mb_per_sec);
        self.total_received_mb.set(ctx, snapshot.total_received_mb);
        self.total_transmitted_mb
            .set(ctx, snapshot.total_transmitted_mb);
        self.system_seconds.set(ctx, snapshot.system_uptime_seconds);
        self.app_seconds.set(ctx, snapshot.app_uptime_seconds);
    }

    fn sync_metric_constraints(
        &self,
        ctx: &mut ProcessCtx,
        metrics: &OsMetricsSnapshot,
    ) {
        sync_value_constraint(
            ctx,
            self.global_ratio.is_bound().then_some(self.global_ratio.id()),
            Some(0.0),
            Some(1.0),
        );
        sync_value_constraint(
            ctx,
            self.app_ratio.is_bound().then_some(self.app_ratio.id()),
            Some(0.0),
            Some(1.0),
        );
        sync_value_constraint(
            ctx,
            self.system_used_mb
                .is_bound()
                .then_some(self.system_used_mb.id()),
            Some(0.0),
            Some(metrics.system_total_mb.max(metrics.system_used_mb)),
        );
        sync_value_constraint(
            ctx,
            self.system_total_mb
                .is_bound()
                .then_some(self.system_total_mb.id()),
            Some(0.0),
            Some(metrics.system_total_mb.max(metrics.system_used_mb).max(metrics.app_used_mb)),
        );
        sync_value_constraint(
            ctx,
            self.app_used_mb
                .is_bound()
                .then_some(self.app_used_mb.id()),
            Some(0.0),
            Some(metrics.system_total_mb.max(metrics.app_used_mb)),
        );
        sync_value_constraint(
            ctx,
            self.app_virtual_mb
                .is_bound()
                .then_some(self.app_virtual_mb.id()),
            Some(0.0),
            None,
        );
        sync_value_constraint(
            ctx,
            self.received_mb_per_sec
                .is_bound()
                .then_some(self.received_mb_per_sec.id()),
            Some(0.0),
            None,
        );
        sync_value_constraint(
            ctx,
            self.transmitted_mb_per_sec
                .is_bound()
                .then_some(self.transmitted_mb_per_sec.id()),
            Some(0.0),
            None,
        );
        sync_value_constraint(
            ctx,
            self.total_received_mb
                .is_bound()
                .then_some(self.total_received_mb.id()),
            Some(0.0),
            None,
        );
        sync_value_constraint(
            ctx,
            self.total_transmitted_mb
                .is_bound()
                .then_some(self.total_transmitted_mb.id()),
            Some(0.0),
            None,
        );
    }

    fn on_custom_event_inner(&mut self, ctx: &mut ProcessCtx, event: CustomEvent) {
        let Some(request) = crate::app::module_command::decode_module_command_request(&event) else {
            return;
        };
        if request.module_id != self.id() || !OS_MODULE_COMMAND_TYPES.contains(&request.command_type.as_str()) {
            return;
        }

        let result = match request.command_type.as_str() {
            OS_SHUTDOWN_COMMAND_NODE_TYPE => serde_json::from_value::<OsControlRequest>(request.payload)
                .map_err(|error| format!("invalid OS shutdown command payload: {error}"))
                .and_then(|payload| self.execute_control_action(ctx, HostControlAction::Shutdown, payload)),
            OS_REBOOT_COMMAND_NODE_TYPE => serde_json::from_value::<OsControlRequest>(request.payload)
                .map_err(|error| format!("invalid OS reboot command payload: {error}"))
                .and_then(|payload| self.execute_control_action(ctx, HostControlAction::Reboot, payload)),
            OS_LOGOUT_COMMAND_NODE_TYPE => serde_json::from_value::<OsControlRequest>(request.payload)
                .map_err(|error| format!("invalid OS logout command payload: {error}"))
                .and_then(|payload| self.execute_control_action(ctx, HostControlAction::Logout, payload)),
            OS_WAKE_ON_LAN_COMMAND_NODE_TYPE => serde_json::from_value::<WakeOnLanRequest>(request.payload)
                .map_err(|error| format!("invalid Wake-on-LAN command payload: {error}"))
                .and_then(|payload| self.execute_wake_on_lan(ctx, payload)),
            _ => Ok(()),
        };

        if let Err(error) = result {
            logerror!(format!("Failed to handle OS command {:?}: {error}", request.command_id));
        }
    }

    fn handle_script_method(
        &mut self,
        ctx: &mut ProcessCtx,
        method: &str,
        args: &[ParamValue],
    ) -> Option<Result<(), String>> {
        let result = match method {
            "shutdown" => script_no_args(method, args).and_then(|_| {
                self.execute_control_action(
                    ctx,
                    HostControlAction::Shutdown,
                    OsControlRequest {
                        description: "shutdown host".to_string(),
                    },
                )
            }),
            "reboot" => script_no_args(method, args).and_then(|_| {
                self.execute_control_action(
                    ctx,
                    HostControlAction::Reboot,
                    OsControlRequest {
                        description: "reboot host".to_string(),
                    },
                )
            }),
            "logout" => script_no_args(method, args).and_then(|_| {
                self.execute_control_action(
                    ctx,
                    HostControlAction::Logout,
                    OsControlRequest {
                        description: "logout host user".to_string(),
                    },
                )
            }),
            "wakeOnLan" => script_wake_on_lan_request(args)
                .and_then(|request| self.execute_wake_on_lan(ctx, request)),
            _ => return None,
        };

        Some(result)
    }

    fn execute_control_action(
        &mut self,
        ctx: &mut ProcessCtx,
        action: HostControlAction,
        request: OsControlRequest,
    ) -> Result<(), String> {
        self.base.emit_outgoing_traffic(ctx);
        self.emit_system_command_requested(
            ctx,
            action.as_script_method(),
            serde_json::json!({
                "description": request.description,
            }),
        );

        if self.base.log_outgoing_enabled() {
            golden_core::log!(origin = self.id(); format!("Requested {}.", action.as_human_label()));
        }

        if let Err(error) = os_runtime::execute_control_action(action) {
            self.emit_system_command_failed(ctx, action.as_script_method(), error.as_str());
            return Err(error);
        }

        Ok(())
    }

    fn execute_wake_on_lan(
        &mut self,
        ctx: &mut ProcessCtx,
        request: WakeOnLanRequest,
    ) -> Result<(), String> {
        self.base.emit_outgoing_traffic(ctx);

        if let Err(error) = os_runtime::send_wake_on_lan(&request) {
            self.emit_system_command_failed(ctx, "wakeOnLan", error.as_str());
            return Err(error);
        }

        self.emit_system_command_requested(
            ctx,
            "wakeOnLan",
            serde_json::json!({
                "description": request.description,
                "macAddress": request.mac_address,
                "broadcastHost": request.broadcast_host,
                "port": request.port,
            }),
        );

        if self.base.log_outgoing_enabled() {
            golden_core::log!(origin = self.id(); format!(
                "Sent Wake-on-LAN packet to {} via {}:{}.",
                request.mac_address,
                request.broadcast_host,
                request.port,
            ));
        }

        Ok(())
    }

    fn emit_system_stats_callback(&self, ctx: &mut ProcessCtx, snapshot: &OsMetricsSnapshot) {
        crate::app::module::script_api::emit_script_callback(
            ctx,
            self.id(),
            SYSTEM_STATS_UPDATED_CALLBACK,
            vec![metrics_json(snapshot)],
        );
    }

    fn emit_system_command_requested(
        &self,
        ctx: &mut ProcessCtx,
        command: &str,
        details: serde_json::Value,
    ) {
        crate::app::module::script_api::emit_script_callback(
            ctx,
            self.id(),
            SYSTEM_COMMAND_REQUESTED_CALLBACK,
            vec![serde_json::json!(command), details],
        );
    }

    fn emit_system_command_failed(&self, ctx: &mut ProcessCtx, command: &str, error: &str) {
        crate::app::module::script_api::emit_script_callback(
            ctx,
            self.id(),
            SYSTEM_COMMAND_FAILED_CALLBACK,
            vec![serde_json::json!(command), serde_json::json!(error)],
        );
    }
}

#[golden_core::item(
    "module",
    node = "os_module",
    via = base,
    from_struct,
    menu_path = ["System"]
)]
impl Node for OsModule {
    fn init(&mut self, ctx: &mut ProcessCtx) {
        self.base.init(ctx);
        self.base.configure_command_tester(ctx, OS_MODULE_COMMAND_TYPES);
        self.base
            .set_data_capabilities(ctx, crate::app::module::ModuleDataCapabilities::new(true, true));
        self.base.set_connected(ctx, true);
        crate::app::module::enable_module_authoring(self.node_data_mut());
        self.refresh_static_values(ctx);
        self.refresh_metrics(ctx);
    }

    fn update(&mut self, ctx: &mut ProcessCtx) {
        if !self.node_data().effective_enabled {
            return;
        }

        self.refresh_metrics(ctx);
    }

    fn destroy(&mut self, _ctx: &mut ProcessCtx) {
        self.runtime.stop();
    }

    fn update_requires_tree_snapshot(&self) -> bool {
        false
    }

    fn execution_rule(&self) -> NodeExecutionRule {
        NodeExecutionRule::periodic(OS_MODULE_UPDATE_RATE_HZ)
            .with_compiled_kernel("chataigne.runtime.os")
    }

    fn engine_script_descriptor(&self) -> NodeScriptDescriptor {
        crate::app::module::script_api::descriptor_for_node(
            self.node_data(),
            self.get_type(),
            OS_SCRIPT_METHODS,
        )
    }

    fn engine_call_script_method(
        &mut self,
        ctx: &mut ProcessCtx,
        method: &str,
        args: &[ParamValue],
    ) -> Result<bool, String> {
        if let Some(result) = self.handle_script_method(ctx, method, args) {
            result?;
            return Ok(true);
        }

        self.base.engine_call_script_method(ctx, method, args)
    }

    fn on_param_change(&mut self, ctx: &mut ProcessCtx, param: NodeId, old_value: ParamValue) {
        if let Some(snapshot_arc) = ctx.tree_snapshot_arc() {
            self.base
                .emit_script_param_callback(ctx, snapshot_arc.as_ref(), param, &old_value);
        }
    }

    fn on_effective_enabled_changed(&mut self, ctx: &mut ProcessCtx, enabled: bool) {
        self.runtime.set_polling_enabled(enabled);
        self.base.set_connected(ctx, enabled);
        if enabled {
            self.refresh_static_values(ctx);
            self.refresh_metrics(ctx);
        }
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: CustomEvent) {
        self.on_custom_event_inner(ctx, event);
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::create)
    }
}

fn script_no_args(method: &str, args: &[ParamValue]) -> Result<(), String> {
    if args.is_empty() {
        return Ok(());
    }

    Err(format!("method '{method}' does not accept arguments"))
}

fn script_wake_on_lan_request(args: &[ParamValue]) -> Result<WakeOnLanRequest, String> {
    let Some(mac_address) = args.first().and_then(ParamValue::as_str) else {
        return Err("method 'wakeOnLan' expects a MAC address string argument".to_string());
    };

    let broadcast_host = args
        .get(1)
        .and_then(ParamValue::as_str)
        .unwrap_or_else(|| "255.255.255.255".to_string());
    let port = args.get(2).and_then(ParamValue::as_int).unwrap_or(9);
    if !(0..=65535).contains(&port) {
        return Err("method 'wakeOnLan' expects a UDP port between 0 and 65535".to_string());
    }

    Ok(WakeOnLanRequest {
        mac_address,
        broadcast_host,
        port: port as u16,
        description: "send Wake-on-LAN packet".to_string(),
    })
}

fn metrics_json(snapshot: &OsMetricsSnapshot) -> serde_json::Value {
    serde_json::json!({
        "info": {
            "osType": os_runtime::host_os_type(),
            "architecture": os_runtime::host_architecture(),
            "osVersion": snapshot.os_version,
        },
        "cpu": {
            "globalRatio": snapshot.global_cpu_ratio,
            "appRatio": snapshot.app_cpu_ratio,
        },
        "memory": {
            "systemUsedMb": snapshot.system_used_mb,
            "systemTotalMb": snapshot.system_total_mb,
            "appUsedMb": snapshot.app_used_mb,
            "appVirtualMb": snapshot.app_virtual_mb,
        },
        "network": {
            "receivedMbPerSec": snapshot.received_mb_per_sec,
            "transmittedMbPerSec": snapshot.transmitted_mb_per_sec,
            "totalReceivedMb": snapshot.total_received_mb,
            "totalTransmittedMb": snapshot.total_transmitted_mb,
        },
        "uptime": {
            "systemSeconds": snapshot.system_uptime_seconds,
            "appSeconds": snapshot.app_uptime_seconds,
        },
    })
}

fn sync_value_constraint(
    ctx: &mut ProcessCtx,
    node_id: Option<NodeId>,
    min: Option<f64>,
    max: Option<f64>,
) {
    let Some(node_id) = node_id else {
        return;
    };

    ctx.edits.push(Edit::SetParamConstraints {
        node: node_id,
        constraints: system_metrics::float_constraints(min, max),
    });
}

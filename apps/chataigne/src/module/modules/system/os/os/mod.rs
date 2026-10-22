mod os_runtime;
mod service;
pub(crate) use service::dispatch as queue_os_command;
#[cfg(test)]
mod tests;

use golden_core::{
    edit::Edit,
    engine::NodeExecutionRule,
    node,
    node::{Node, NodeId, NodeScriptDescriptor},
    parameter::ParamValue,
    process_ctx::ProcessCtx,
};

use crate::app::module::common::os::{
    OS_LOGOUT_COMMAND_NODE_TYPE, OS_MODULE_COMMAND_TYPES, OS_REBOOT_COMMAND_NODE_TYPE, OS_SHUTDOWN_COMMAND_NODE_TYPE,
    OS_WAKE_ON_LAN_COMMAND_NODE_TYPE,
};
use crate::app::module::common::system_metrics;

use self::os_runtime::{OsMetricsSnapshot, OsRuntime};

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
    runtime: Option<OsRuntime>,
    wake: Option<crate::app::module::common::worker_wake::ModuleWake>,
    retry_at: Option<std::time::Instant>,
    reconnect: golden_io::ReconnectBackoff,
    pending: std::collections::VecDeque<os_runtime::Receipt>,
    outgoing: std::collections::VecDeque<golden_io::RetainedPayload<os_runtime::Source>>,
    admission: golden_io::RetirementPool,
    output_retry_at: Option<std::time::Instant>,
}

impl OsModule {
    pub fn create() -> Self {
        Self::new(
            crate::app::ModuleBase::new(),
            None,
            None,
            None,
            golden_io::ReconnectBackoff::new(std::time::Duration::from_millis(250), std::time::Duration::from_secs(8)),
            std::collections::VecDeque::new(),
            std::collections::VecDeque::new(),
            golden_io::RetirementPool::new(std::num::NonZeroUsize::new(64).unwrap()),
            None,
        )
    }

    fn refresh_static_values(&mut self, ctx: &mut ProcessCtx) {
        self.os_type.set(ctx, os_runtime::host_os_type().to_string());
        self.architecture.set(ctx, os_runtime::host_architecture());
    }

    fn apply_metrics_snapshot(&mut self, ctx: &mut ProcessCtx, snapshot: &OsMetricsSnapshot) {
        self.architecture.set(ctx, snapshot.architecture.clone());
        self.os_version.set(ctx, snapshot.os_version.clone());
        self.global_ratio.set(ctx, snapshot.global_cpu_ratio);
        self.app_ratio.set(ctx, snapshot.app_cpu_ratio);
        self.system_used_mb.set(ctx, snapshot.system_used_mb);
        self.system_total_mb.set(ctx, snapshot.system_total_mb);
        self.app_used_mb.set(ctx, snapshot.app_used_mb);
        self.app_virtual_mb.set(ctx, snapshot.app_virtual_mb);
        self.received_mb_per_sec.set(ctx, snapshot.received_mb_per_sec);
        self.transmitted_mb_per_sec.set(ctx, snapshot.transmitted_mb_per_sec);
        self.total_received_mb.set(ctx, snapshot.total_received_mb);
        self.total_transmitted_mb.set(ctx, snapshot.total_transmitted_mb);
        self.system_seconds.set(ctx, snapshot.system_uptime_seconds);
        self.app_seconds.set(ctx, snapshot.app_uptime_seconds);
    }

    fn sync_metric_constraints(&self, ctx: &mut ProcessCtx, metrics: &OsMetricsSnapshot) {
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
            self.system_used_mb.is_bound().then_some(self.system_used_mb.id()),
            Some(0.0),
            Some(metrics.system_total_mb.max(metrics.system_used_mb)),
        );
        sync_value_constraint(
            ctx,
            self.system_total_mb.is_bound().then_some(self.system_total_mb.id()),
            Some(0.0),
            Some(
                metrics
                    .system_total_mb
                    .max(metrics.system_used_mb)
                    .max(metrics.app_used_mb),
            ),
        );
        sync_value_constraint(
            ctx,
            self.app_used_mb.is_bound().then_some(self.app_used_mb.id()),
            Some(0.0),
            Some(metrics.system_total_mb.max(metrics.app_used_mb)),
        );
        sync_value_constraint(
            ctx,
            self.app_virtual_mb.is_bound().then_some(self.app_virtual_mb.id()),
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
            self.total_received_mb.is_bound().then_some(self.total_received_mb.id()),
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
        self.base.set_connected(ctx, false);
        crate::app::module::enable_module_authoring(self.node_data_mut());
        self.refresh_static_values(ctx);
    }

    fn on_node_ready(&mut self, ctx: &mut ProcessCtx, context: golden_core::node::NodeCreationContext) {
        self.ready(ctx, context);
    }
    fn update(&mut self, ctx: &mut ProcessCtx) {
        self.service(ctx);
    }
    fn destroy(&mut self, _ctx: &mut ProcessCtx) {
        self.clear_outgoing();
        self.runtime = None;
        self.wake = None;
    }
    fn needs_update(&self) -> bool {
        false
    }
    fn update_requires_tree_snapshot(&self) -> bool {
        self.needs_service_snapshot()
    }
    fn execution_rule(&self) -> NodeExecutionRule {
        NodeExecutionRule::passive().with_compiled_kernel("chataigne.runtime.os")
    }
    fn on_worker_wake(&mut self, ctx: &mut ProcessCtx) {
        self.service(ctx);
    }
    fn worker_wake_requires_tree_snapshot(&self) -> bool {
        self.needs_service_snapshot()
    }
    fn worker_wake_when_disabled(&self) -> bool {
        !self.pending.is_empty()
    }

    fn engine_script_descriptor(&self) -> NodeScriptDescriptor {
        crate::app::module::script_api::descriptor_for_node(self.node_data(), self.get_type(), OS_SCRIPT_METHODS)
    }

    fn engine_call_script_method(
        &mut self,
        ctx: &mut ProcessCtx,
        method: &str,
        args: &[ParamValue],
    ) -> Result<bool, String> {
        if let Some(result) = self.script(ctx, method, args) {
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
        self.retry_at = None;
        if enabled {
            if let Err(error) = self.register_service(ctx) {
                golden_core::logerror!(error);
            }
            self.request_service();
        } else {
            self.clear_outgoing();
            self.runtime = None;
            if let Some(wake) = &self.wake {
                wake.cancel_deadline();
            }
            self.base.set_connected(ctx, false);
            if self.pending.iter().any(os_runtime::Receipt::complete) {
                self.request_service();
            }
        }
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: golden_core::events::CustomEvent) {
        self.accept_output(ctx, event);
    }
    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::create)
    }
}

fn metrics_json(snapshot: &OsMetricsSnapshot) -> serde_json::Value {
    serde_json::json!({
        "info": {
            "osType": os_runtime::host_os_type(),
            "architecture": snapshot.architecture,
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

fn sync_value_constraint(ctx: &mut ProcessCtx, node_id: Option<NodeId>, min: Option<f64>, max: Option<f64>) {
    let Some(node_id) = node_id else {
        return;
    };

    ctx.edits.push(Edit::SetParamConstraints {
        node: node_id,
        constraints: system_metrics::float_constraints(min, max),
    });
}

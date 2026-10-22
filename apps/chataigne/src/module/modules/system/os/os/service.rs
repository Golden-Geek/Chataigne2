use super::*;
use golden_core::{
    edit::{EditPreconditions, NodeReadGuard},
    node::NodeCreationContext,
};
use golden_io::RetainedPayload;
use os_runtime::{Input, Method, Outcome, Receipt, Source, Status};
use std::time::Instant;
const OUTPUT_TOPIC: &str = "chataigne.os.captured_output";

impl OsModule {
    pub(super) fn register_service(&mut self, ctx: &ProcessCtx) -> Result<(), String> {
        let source = match ctx.node_waker() {
            Ok(source) => source,
            Err(golden_core::engine::NodeWakeError::Unavailable) => return Ok(()),
            Err(error) => return Err(error.to_string()),
        };
        if !self.wake.as_ref().is_some_and(|wake| wake.matches(&source)) {
            self.clear_outgoing();
            self.runtime = None;
            self.retry_at = None;
            self.wake = Some(crate::app::module::common::worker_wake::ModuleWake::new(source)?);
        }
        Ok(())
    }
    pub(super) fn request_service(&self) {
        if let Some(wake) = &self.wake {
            wake.notifier.notify();
        }
    }
    fn fail_runtime(&mut self, ctx: &mut ProcessCtx, error: &str) {
        self.runtime = None;
        self.output_retry_at = None;
        if !self.outgoing.is_empty() {
            self.request_service();
        }
        self.base.set_connected(ctx, false);
        self.warning(ctx, error);
        if self.node_data().effective_enabled {
            let at = self.reconnect.schedule(Instant::now());
            self.retry_at = Some(at);
            if let Some(wake) = &self.wake {
                wake.later(at.saturating_duration_since(Instant::now()));
            }
        }
    }
    fn warning(&self, ctx: &mut ProcessCtx, message: &str) {
        if let Some(id) = self.base.connection_id() {
            golden_core::node::NodeHandle::new(id).set_warning_with(ctx, Some("os_runtime"), message, None);
        }
    }
    pub(super) fn service(&mut self, ctx: &mut ProcessCtx) {
        for _ in 0..self.pending.len().min(16) {
            let receipt = self.pending.pop_front().unwrap();
            if ctx.tree_snapshot().is_none() {
                if receipt.complete() {
                    self.request_service();
                }
                self.pending.push_back(receipt);
                continue;
            }
            match receipt.take() {
                Some(Ok(outcome)) => self.project_outcome(ctx, outcome),
                Some(Err(error)) => {
                    self.warning(ctx, &error);
                    golden_core::logerror!(error);
                }
                None => self.pending.push_back(receipt),
            }
        }
        if !self.node_data().effective_enabled {
            return;
        }
        if self.runtime.is_none() && self.retry_at.is_none_or(|at| Instant::now() >= at) {
            self.retry_at = None;
            let Some(wake) = &self.wake else {
                return;
            };
            match OsRuntime::spawn(wake.notifier.clone()) {
                Ok(runtime) => self.runtime = Some(runtime),
                Err(error) => {
                    self.fail_runtime(ctx, &error);
                    return;
                }
            }
        }
        self.submit_outputs(ctx);
        let Some(runtime) = &self.runtime else {
            return;
        };
        let stopped = !runtime.is_running();
        let mut status = Vec::with_capacity(4);
        for _ in 0..4 {
            if let Some(value) = runtime.take_status() {
                status.push(value);
            } else {
                break;
            }
        }
        let metrics = if ctx.tree_snapshot().is_some() {
            runtime.take_metrics()
        } else {
            None
        };
        for value in status {
            match value {
                Status::Ready => {
                    self.base.set_connected(ctx, true);
                    self.retry_at = None;
                    if let Some(id) = self.base.connection_id() {
                        golden_core::node::NodeHandle::new(id).clear_warning(ctx, Some("os_runtime"));
                    }
                }
                Status::Warning(message) => self.warning(ctx, &message),
                Status::Error(error) => {
                    self.fail_runtime(ctx, &error);
                    return;
                }
            }
        }
        if stopped {
            self.fail_runtime(ctx, "OS native worker stopped unexpectedly");
            return;
        }
        if let Some(metrics) = metrics {
            self.project_metrics(ctx, metrics);
        }
        if self.runtime.as_ref().is_some_and(OsRuntime::pending) || self.pending.iter().any(Receipt::complete) {
            self.request_service();
        }
    }
    pub(super) fn project_metrics(&mut self, ctx: &mut ProcessCtx, metrics: RetainedPayload<os_runtime::Metrics>) {
        let Some(snapshot) = ctx.tree_snapshot_arc() else {
            return;
        };
        self.reconnect.reset();
        let mut guards = Vec::with_capacity(15);
        for id in [
            self.id(),
            self.architecture.id(),
            self.os_version.id(),
            self.global_ratio.id(),
            self.app_ratio.id(),
            self.system_used_mb.id(),
            self.system_total_mb.id(),
            self.app_used_mb.id(),
            self.app_virtual_mb.id(),
            self.received_mb_per_sec.id(),
            self.transmitted_mb_per_sec.id(),
            self.total_received_mb.id(),
            self.total_transmitted_mb.id(),
            self.system_seconds.id(),
            self.app_seconds.id(),
        ] {
            let Some(guard) = NodeReadGuard::capture_structure(&snapshot, id) else {
                return;
            };
            guards.push(guard);
        }
        ctx.edits.push(Edit::CheckPreconditions {
            preconditions: EditPreconditions::new(guards, metrics.fence.guards()).unwrap(),
        });
        self.sync_metric_constraints(ctx, &metrics.snapshot);
        self.apply_metrics_snapshot(ctx, &metrics.snapshot);
        self.base.emit_incoming_traffic(ctx);
        ctx.emit_custom_event(metrics.callback.event(self.id()));
        if self.base.log_incoming_enabled() {
            golden_core::log!(origin = self.id(); format!("System stats refreshed: CPU {:.3}, memory {:.1} MB", metrics.snapshot.global_cpu_ratio, metrics.snapshot.system_used_mb));
        }
        ctx.edits.retain_payload(metrics);
    }
    fn project_outcome(&mut self, ctx: &mut ProcessCtx, outcome: RetainedPayload<Outcome>) {
        let Some(snapshot) = ctx.tree_snapshot_arc() else {
            return;
        };
        let Some(guard) = NodeReadGuard::capture_identity(&snapshot, self.id()) else {
            return;
        };
        ctx.edits.push(Edit::CheckPreconditions {
            preconditions: EditPreconditions::new(vec![guard], vec![]).unwrap(),
        });
        if outcome.attempted {
            self.base.emit_outgoing_traffic(ctx);
        }
        if let Some(callback) = &outcome.requested {
            ctx.emit_custom_event(callback.event(self.id()));
        }
        if let Some(callback) = &outcome.failed {
            ctx.emit_custom_event(callback.event(self.id()));
        }
        if let Some(error) = &outcome.error {
            self.warning(ctx, error);
            golden_core::logerror!(error);
        }
        if let Some(error) = &outcome.callback_error {
            self.warning(ctx, error);
            golden_core::logerror!(error);
        }
        if self.base.log_outgoing_enabled() {
            golden_core::log!(origin = self.id(); format!("OS {} native outcome: {}", outcome.source.method.name(), outcome.error.as_deref().unwrap_or("submitted locally")));
        }
        ctx.edits.retain_payload(outcome);
    }
    pub(super) fn queue(
        &mut self,
        ctx: &mut ProcessCtx,
        input: Input,
        producer: golden_core::engine::NodeWaker,
        method: Method,
        time: golden_core::engine::EngineTime,
    ) -> Result<(), String> {
        if !self.node_data().effective_enabled || self.pending.len() >= 64 {
            return Err("OS module is disabled or its 64 pending outcomes are occupied".into());
        }
        let runtime = self
            .runtime
            .as_ref()
            .ok_or("OS runtime is opening or stopped; request was not admitted")?;
        let permit = self
            .admission
            .try_reserve()
            .map_err(|_| "OS module's 64 source lifetimes are occupied")?;
        let source = runtime.capture(input, producer, method, time, Some(permit))?;
        // Inbox-only attachments keep command and direct-script capture on the
        // same stabilization boundary, before the next native dispatch turn.
        ctx.emit_custom_event(golden_core::events::CustomEvent::local(
            OUTPUT_TOPIC,
            Some(self.id()),
            source,
        ));
        Ok(())
    }
    pub(super) fn script(
        &mut self,
        ctx: &mut ProcessCtx,
        method: &str,
        args: &[ParamValue],
    ) -> Option<Result<(), String>> {
        let method = Method::parse(method)?;
        Some((|| {
            let args = ctx.capture_script_args(args).map_err(str::to_owned)?;
            if args.len() > 3 || args.retained_bytes() > 64 * 1024 {
                return Err("OS scripts accept at most 3 arguments / 64 KiB retained storage".into());
            }
            let producer = ctx.node_waker().map_err(|error| error.to_string())?;
            let time = ctx.time;
            self.queue(ctx, Input::Script { method, args }, producer, method, time)
        })())
    }
    pub(super) fn accept_output(&mut self, ctx: &mut ProcessCtx, event: golden_core::events::CustomEvent) {
        if event.topic != OUTPUT_TOPIC || event.origin != Some(self.id()) {
            return;
        }
        let Some(source) = event.local_payload::<RetainedPayload<Source>>() else {
            return;
        };
        if !self.node_data().effective_enabled || !source.live() || self.outgoing.len() + self.pending.len() >= 64 {
            self.warning(
                ctx,
                "OS captured source retired or queue occupied; request was not sent",
            );
            golden_core::logerror!("OS captured source retired or queue occupied; request was not sent");
            return;
        }
        let index = self.outgoing.partition_point(|pending| pending.time <= source.time);
        self.outgoing.insert(index, source.clone());
        self.request_service();
    }
    fn submit_outputs(&mut self, ctx: &mut ProcessCtx) {
        if self.output_retry_at.is_some_and(|at| Instant::now() < at) {
            return;
        }
        self.output_retry_at = None;
        for _ in 0..8 {
            let Some(source) = self.outgoing.pop_front() else {
                break;
            };
            if !source.live() {
                self.warning(ctx, "OS source retired before native admission; request was not sent");
                golden_core::logerror!("OS source retired before native admission; request was not sent");
                continue;
            }
            let Some(runtime) = &self.runtime else {
                self.outgoing.push_front(source);
                self.request_service();
                return;
            };
            match runtime.submit(source) {
                Ok(receipt) => self.pending.push_back(receipt),
                Err(source) => {
                    self.outgoing.push_front(source);
                    self.output_retry_at = Some(Instant::now() + std::time::Duration::from_millis(250));
                    if let Some(wake) = &self.wake {
                        wake.later(std::time::Duration::from_millis(250));
                    }
                    return;
                }
            }
        }
        if !self.outgoing.is_empty() {
            self.request_service();
        }
    }
    pub(super) fn clear_outgoing(&mut self) {
        if !self.outgoing.is_empty() {
            golden_core::logerror!(format!(
                "OS retired {} captured requests; they were not sent",
                self.outgoing.len()
            ));
        }
        self.outgoing.clear();
        self.output_retry_at = None;
    }
    pub(super) fn needs_service_snapshot(&self) -> bool {
        self.runtime.as_ref().is_some_and(OsRuntime::has_metrics) || self.pending.iter().any(Receipt::complete)
    }
    pub(super) fn ready(&mut self, ctx: &mut ProcessCtx, _context: NodeCreationContext) {
        if let Err(error) = self.register_service(ctx) {
            self.warning(ctx, &error);
            return;
        }
        self.refresh_static_values(ctx);
        self.request_service();
    }
}

pub(crate) fn dispatch(
    ctx: &mut ProcessCtx,
    invocation: crate::app::module_command::CapturedInvocation,
) -> Result<(), String> {
    let method = match invocation
        .snapshot
        .node(invocation.command)
        .map(|node| node.node_type.as_str())
    {
        Some(OS_SHUTDOWN_COMMAND_NODE_TYPE) => Method::Shutdown,
        Some(OS_REBOOT_COMMAND_NODE_TYPE) => Method::Reboot,
        Some(OS_LOGOUT_COMMAND_NODE_TYPE) => Method::Logout,
        Some(OS_WAKE_ON_LAN_COMMAND_NODE_TYPE) => Method::WakeOnLan,
        _ => return Err("Unsupported OS command type".into()),
    };
    let producer = ctx.node_waker().map_err(|error| error.to_string())?;
    let time = ctx.current_event().map_or(ctx.time, |event| event.time);
    ctx.call_node_mutation_without_snapshot(invocation.module, move |node, ctx| {
        let module = node
            .as_any_mut()
            .downcast_mut::<OsModule>()
            .ok_or("OS command target is unavailable")?;
        if let Err(error) = module.queue(ctx, Input::Command(invocation), producer, method, time) {
            module.warning(ctx, &error);
            golden_core::logerror!(error);
        }
        Ok(())
    });
    Ok(())
}

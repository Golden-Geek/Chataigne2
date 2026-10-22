use super::*;
use crate::app::module::common::worker_wake::ModuleWake;
use app_control_runtime::{AppControlRuntime, Config, Event as NativeEvent, Input, Outcome, Receipt, Report, Source};
use golden_core::edit::{Edit, EditPreconditions, NodeReadGuard};
use golden_io::{ReconnectBackoff, RetainedPayload, RetirementPool, RevisionSource};
use std::{
    collections::VecDeque,
    num::NonZeroUsize,
    time::{Duration, Instant},
};
#[cfg(test)]
mod tests;

const OUTPUT_TOPIC: &str = "chataigne.app-control.captured-output";
pub(super) struct Service {
    pub(super) runtime: Option<AppControlRuntime>,
    pub(super) wake: Option<ModuleWake>,
    pub(super) config: Option<RetainedPayload<Config>>,
    pub(super) report: Option<(RetainedPayload<Report>, usize)>,
    pub(super) pending: VecDeque<Receipt>,
    outgoing: VecDeque<RetainedPayload<Source>>,
    admission: RetirementPool,
    retry_at: Option<Instant>,
    output_retry_at: Option<Instant>,
    pub(super) config_retry_at: Option<Instant>,
    pub(super) reconnect: ReconnectBackoff,
    pub(super) structure_pending: bool,
    pub(super) catalog: Option<catalog::Cursor>,
    pub(super) catalog_generation: RevisionSource,
}
impl Service {
    pub(super) fn new() -> Self {
        Self {
            runtime: None,
            wake: None,
            config: None,
            report: None,
            pending: VecDeque::new(),
            outgoing: VecDeque::new(),
            admission: RetirementPool::new(NonZeroUsize::new(64).unwrap()),
            retry_at: None,
            output_retry_at: None,
            config_retry_at: None,
            reconnect: ReconnectBackoff::new(Duration::from_millis(250), Duration::from_secs(8)),
            structure_pending: false,
            catalog: None,
            catalog_generation: RevisionSource::default(),
        }
    }
    pub(super) fn retire(&mut self) {
        self.runtime = None;
        self.config = None;
        self.report = None;
        self.catalog = None;
        self.catalog_generation.advance();
        self.retry_at = None;
        self.output_retry_at = None;
        self.config_retry_at = None;
        if !self.outgoing.is_empty() {
            golden_core::logerror!(format!(
                "App Control retired {} captured requests; they were not sent",
                self.outgoing.len()
            ));
        }
        self.outgoing.clear();
        if let Some(wake) = &self.wake {
            wake.cancel_deadline();
        }
    }
}
impl AppControlModule {
    pub(super) fn register_service(&mut self, ctx: &ProcessCtx) -> Result<(), String> {
        let source = match ctx.node_waker() {
            Ok(value) => value,
            Err(golden_core::engine::NodeWakeError::Unavailable) => return Ok(()),
            Err(error) => return Err(error.to_string()),
        };
        if !self.service.wake.as_ref().is_some_and(|wake| wake.matches(&source)) {
            self.service.retire();
            self.watch_config_dirty = true;
            self.service.wake = Some(ModuleWake::new(source)?);
        }
        Ok(())
    }
    pub(super) fn request_service(&self) {
        if let Some(wake) = &self.service.wake {
            wake.notifier.notify();
        }
    }
    pub(super) fn warning(&self, ctx: &mut ProcessCtx, message: &str) {
        if let Some(id) = self.base.connection_id() {
            NodeHandle::new(id).set_warning_with(ctx, Some("app_control_runtime"), message, None);
        }
    }
    fn fail_runtime(&mut self, ctx: &mut ProcessCtx, error: &str) {
        self.service.runtime = None;
        self.service.config = None;
        self.service.report = None;
        self.service.output_retry_at = None;
        self.service.config_retry_at = None;
        self.watch_config_dirty = true;
        self.base.set_connected(ctx, false);
        self.warning(ctx, error);
        if self.node_data().effective_enabled {
            let at = self.service.reconnect.schedule(Instant::now());
            self.service.retry_at = Some(at);
            if let Some(wake) = &self.service.wake {
                wake.later(at.saturating_duration_since(Instant::now()));
            }
        }
    }
    pub(super) fn service(&mut self, ctx: &mut ProcessCtx) {
        for _ in 0..self.service.pending.len().min(16) {
            let receipt = self.service.pending.pop_front().unwrap();
            if ctx.tree_snapshot().is_none() {
                if receipt.complete() {
                    self.request_service();
                }
                self.service.pending.push_back(receipt);
                continue;
            }
            match receipt.take() {
                Some(Ok(value)) => self.project_outcome(ctx, value),
                Some(Err(error)) => {
                    self.warning(ctx, &error);
                    golden_core::logerror!(error);
                }
                None => self.service.pending.push_back(receipt),
            }
        }
        if self.service.pending.iter().any(Receipt::complete) {
            self.request_service();
        }
        if !self.node_data().effective_enabled {
            return;
        }
        if self.service.runtime.is_none() && self.service.retry_at.is_none_or(|at| Instant::now() >= at) {
            self.service.retry_at = None;
            let Some(wake) = &self.service.wake else {
                return;
            };
            match AppControlRuntime::spawn(wake.notifier.clone()) {
                Ok(runtime) => {
                    self.service.runtime = Some(runtime);
                    self.watch_config_dirty = true;
                }
                Err(error) => {
                    self.fail_runtime(ctx, &error);
                    return;
                }
            }
        }
        let Some(snapshot) = ctx.tree_snapshot_arc() else {
            self.request_service();
            return;
        };
        if self.watch_config_dirty
            && self.service.runtime.is_some()
            && self.service.config_retry_at.is_none_or(|at| Instant::now() >= at)
        {
            self.service.config_retry_at = None;
            self.sync_runtime_state(ctx, &snapshot);
        } else if self.service.structure_pending {
            if let Some(config) = self.service.config.clone() {
                self.sync_value_structure(ctx, &snapshot, &config.apps, &config.folders);
            }
        }
        self.service_catalog(ctx, &snapshot);
        self.submit_outputs(ctx);
        if self.service.report.is_none() {
            for _ in 0..4 {
                let Some(event) = self.service.runtime.as_ref().and_then(AppControlRuntime::event) else {
                    break;
                };
                match event {
                    NativeEvent::Ready => {
                        self.base.set_connected(ctx, true);
                        if let Some(id) = self.base.connection_id() {
                            NodeHandle::new(id).clear_warning(ctx, Some("app_control_runtime"));
                        }
                    }
                    NativeEvent::Warning(error) => self.warning(ctx, &error),
                    NativeEvent::Error(error) => {
                        self.fail_runtime(ctx, &error);
                        return;
                    }
                    NativeEvent::Report(report) => {
                        self.service.report = Some((report, 0));
                        break;
                    }
                }
            }
        }
        if self.service.runtime.as_ref().is_some_and(|runtime| !runtime.running()) {
            self.fail_runtime(ctx, "App Control native worker stopped unexpectedly");
            return;
        }
        self.project_report(ctx, &snapshot);
        if self.service.report.is_some()
            || self.service.catalog.is_some()
            || self.service.structure_pending
            || self.service.runtime.as_ref().is_some_and(AppControlRuntime::pending)
        {
            self.request_service();
        }
        // ModuleWake retains one earliest deadline. Re-arm other absolute
        // deadlines after an earlier retry consumes that shared notification.
        if let Some(at) = [
            self.service.retry_at,
            self.service.config_retry_at,
            self.service.output_retry_at,
        ]
        .into_iter()
        .flatten()
        .min()
        {
            if let Some(wake) = &self.service.wake {
                wake.later(at.saturating_duration_since(Instant::now()));
            }
        }
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
        for error in [&outcome.error, &outcome.callback_error].into_iter().flatten() {
            self.warning(ctx, error);
            golden_core::logerror!(error);
        }
        if self.base.log_outgoing_enabled() {
            golden_core::log!(origin=self.id();format!("App Control {} native outcome: {}",outcome.source.method(),outcome.error.as_deref().unwrap_or("submitted locally")));
        }
        ctx.edits.retain_payload(outcome);
    }
    pub(super) fn queue(
        &mut self,
        ctx: &mut ProcessCtx,
        input: Input,
        producer: golden_core::engine::NodeWaker,
        time: golden_core::engine::EngineTime,
    ) -> Result<(), String> {
        if !self.node_data().effective_enabled {
            return Err("App Control module is disabled".into());
        }
        let runtime = self
            .service
            .runtime
            .as_ref()
            .ok_or("App Control runtime is opening or stopped; request was not admitted")?;
        let permit = self
            .service
            .admission
            .try_reserve()
            .map_err(|_| "App Control module's 64 source lifetimes are occupied")?;
        let source = runtime.capture(input, producer, time, permit)?;
        ctx.emit_custom_event(CustomEvent::local(OUTPUT_TOPIC, Some(self.id()), source));
        Ok(())
    }
    pub(super) fn capture_script(
        &mut self,
        ctx: &mut ProcessCtx,
        method: &'static str,
        args: &[ParamValue],
    ) -> Result<(), String> {
        let args = ctx.capture_script_args(args).map_err(str::to_owned)?;
        if args.len() > 9 || args.retained_bytes() > 64 * 1024 {
            return Err("App Control scripts accept at most 9 arguments / 64 KiB retained storage".into());
        }
        let snapshot = ctx
            .tree_snapshot_arc()
            .ok_or("Missing App Control script snapshot")?
            .try_lease()
            .map_err(|error| error.to_string())?;
        let producer = ctx.node_waker().map_err(|error| error.to_string())?;
        self.queue(
            ctx,
            Input::Script {
                module: self.id(),
                method,
                args,
                snapshot,
            },
            producer,
            ctx.time,
        )
    }
    pub(super) fn capture_running(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &std::sync::Arc<ProcessTreeSnapshot>,
        param: NodeId,
    ) {
        let Some(folder) = snapshot
            .node(param)
            .filter(|node| node.decl_id.rsplit('/').next() == Some("running") || node.short_name == "running")
            .and_then(|node| node.parent)
        else {
            return;
        };
        let Some(item) = self
            .watched_app_value_folders
            .iter()
            .find_map(|(item, value)| (*value == folder).then_some(*item))
        else {
            return;
        };
        let Some(event) = ctx.current_event() else {
            return;
        };
        let EventKind::ParamChanged {
            new_value: ParamValue::Bool(desired),
            source,
            ..
        } = &event.kind
        else {
            return;
        };
        if *source == Some(self.id()) {
            return;
        }
        let (desired, time) = (*desired, event.time);
        let result = (|| {
            let snapshot = snapshot.try_lease().map_err(|error| error.to_string())?;
            let producer = ctx.node_waker().map_err(|error| error.to_string())?;
            self.queue(
                ctx,
                Input::Running {
                    snapshot,
                    item,
                    desired,
                },
                producer,
                time,
            )
        })();
        if let Err(error) = result {
            self.warning(ctx, &error);
            golden_core::logerror!(error);
        }
    }
    pub(super) fn accept_output(&mut self, ctx: &mut ProcessCtx, event: CustomEvent) {
        if event.topic != OUTPUT_TOPIC || event.origin != Some(self.id()) {
            return;
        }
        let Some(source) = event.local_payload::<RetainedPayload<Source>>() else {
            return;
        };
        if !self.node_data().effective_enabled || !source.live() {
            self.warning(ctx, "App Control captured source retired; request was not sent");
            return;
        }
        let at = self
            .service
            .outgoing
            .partition_point(|pending| pending.time <= source.time);
        self.service.outgoing.insert(at, source.clone());
        self.request_service();
    }
    fn submit_outputs(&mut self, ctx: &mut ProcessCtx) {
        if self.service.output_retry_at.is_some_and(|at| Instant::now() < at) {
            return;
        }
        self.service.output_retry_at = None;
        for _ in 0..8 {
            let Some(source) = self.service.outgoing.pop_front() else {
                break;
            };
            if !source.live() {
                self.warning(
                    ctx,
                    "App Control source retired before native admission; request was not sent",
                );
                continue;
            }
            let Some(runtime) = &self.service.runtime else {
                self.service.outgoing.push_front(source);
                return;
            };
            match runtime.submit(source) {
                Ok(receipt) => self.service.pending.push_back(receipt),
                Err(source) => {
                    self.service.outgoing.push_front(source);
                    let delay = Duration::from_millis(250);
                    self.service.output_retry_at = Some(Instant::now() + delay);
                    if let Some(wake) = &self.service.wake {
                        wake.later(delay);
                    }
                    return;
                }
            }
        }
        if !self.service.outgoing.is_empty() {
            self.request_service();
        }
    }
}
pub(crate) fn dispatch(
    ctx: &mut ProcessCtx,
    invocation: crate::app::module_command::CapturedInvocation,
) -> Result<(), String> {
    let producer = ctx.node_waker().map_err(|error| error.to_string())?;
    let time = ctx.current_event().map_or(ctx.time, |event| event.time);
    ctx.call_node_mutation_without_snapshot(invocation.module, move |node, ctx| {
        let module = node
            .as_any_mut()
            .downcast_mut::<AppControlModule>()
            .ok_or("App Control command target is unavailable")?;
        if let Err(error) = module.queue(ctx, Input::Command(invocation), producer, time) {
            module.warning(ctx, &error);
            golden_core::logerror!(error);
        }
        Ok(())
    });
    Ok(())
}

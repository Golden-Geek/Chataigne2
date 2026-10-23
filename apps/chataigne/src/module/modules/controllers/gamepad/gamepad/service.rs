use super::*;
use crate::app::module::common::worker_wake::ModuleWake;
use golden_io::{ReconnectBackoff, RetainedPayload, RetirementPool};
use std::{
    num::NonZeroUsize,
    time::{Duration, Instant},
};
pub(super) struct Service {
    pub(super) runtime: Option<GamepadRuntime>,
    pub(super) owner: RetirementPool,
    pub(super) catalog: Option<RetainedPayload<gamepad_runtime::Publication>>,
    wake: Option<ModuleWake>,
    pub(super) retry_at: Option<Instant>,
    backoff: ReconnectBackoff,
    last_error: Option<String>,
    pressure: bool,
    pub(super) suppressed: bool,
    #[cfg(test)]
    pub(super) factory: Option<Factory>,
}
#[cfg(test)]
type Factory = Box<
    dyn Fn(&RetirementPool, golden_io::WorkerNotifier, gamepad_runtime::Settings) -> Result<GamepadRuntime, String>
        + Send
        + Sync,
>;
impl Service {
    pub(super) fn new() -> Self {
        Self {
            runtime: None,
            owner: RetirementPool::new(NonZeroUsize::MIN),
            catalog: None,
            wake: None,
            retry_at: None,
            backoff: ReconnectBackoff::new(Duration::from_millis(250), Duration::from_secs(5)),
            last_error: None,
            pressure: false,
            suppressed: false,
            #[cfg(test)]
            factory: None,
        }
    }
}
impl GamepadModule {
    pub(super) fn register_service(&mut self, ctx: &ProcessCtx) -> Result<(), String> {
        let waker = match ctx.node_waker() {
            Ok(w) => w,
            Err(golden_core::engine::NodeWakeError::Unavailable) => return Ok(()),
            Err(e) => return Err(e.to_string()),
        };
        if !self.service.wake.as_ref().is_some_and(|w| w.matches(&waker)) {
            self.stop_runtime();
            self.service.wake = Some(ModuleWake::new(waker)?);
        }
        Ok(())
    }
    pub(super) fn request_service(&self) {
        if let Some(w) = &self.service.wake {
            w.notifier.notify();
        }
    }
    pub(super) fn stop_runtime(&mut self) {
        self.service.runtime = None;
        self.service.catalog = None;
        self.service.pressure = false;
        self.service.retry_at = None;
        if let Some(w) = &self.service.wake {
            w.cancel_deadline();
        }
    }
    pub(super) fn configure_runtime(&mut self, ctx: &mut ProcessCtx) -> bool {
        let axes = self.axis_config();
        if let Some(runtime) = self.service.runtime.as_mut() {
            if let Err(error) = runtime.configure(self.device.get_ref().as_str(), axes) {
                self.failed(ctx, error);
                return false;
            }
        }
        true
    }
    fn failed(&mut self, ctx: &mut ProcessCtx, error: String) {
        self.stop_runtime();
        if self.service.last_error.as_ref() != Some(&error) {
            golden_core::logerror!(origin=self.id();format!("Gamepad input stopped: {error}"));
            self.service.last_error = Some(error.clone());
        }
        if self.guard_targets(ctx) {
            self.set_runtime_warning(ctx, &error);
            self.device.clear_warning(ctx, Some("gamepad_pressure"));
            self.project_metadata(ctx, None, 0);
            self.reset_values(ctx);
            sync_gamepad_device_enum_options(ctx, self.device.id(), gamepad_device_options(&[]));
        }
        self.service.retry_at = Some(self.service.backoff.schedule(Instant::now()));
    }
    fn start_runtime(&self) -> Result<GamepadRuntime, String> {
        let notifier = self
            .service
            .wake
            .as_ref()
            .ok_or("Gamepad wake unavailable")?
            .notifier
            .clone();
        #[cfg(test)]
        if let Some(factory) = &self.service.factory {
            return factory(&self.service.owner, notifier, self.settings());
        }
        GamepadRuntime::create(&self.service.owner, notifier, self.settings())
    }
    pub(super) fn service_runtime(&mut self, ctx: &mut ProcessCtx) {
        if ctx.tree_snapshot_arc().is_none() {
            return;
        }
        if let Err(error) = self.register_service(ctx) {
            self.set_runtime_warning(ctx, &error);
            return;
        }
        if !self.node_data().effective_enabled {
            self.stop_runtime();
            return;
        }
        if self.device.get_ref().as_str().len() > 1024 {
            self.failed(ctx, "Gamepad selection exceeds 1024 bytes".into());
            return;
        }
        if !self.configure_runtime(ctx) {
            return;
        }
        if self.service.runtime.is_none()
            && !self.service.suppressed
            && !self.service.retry_at.is_some_and(|at| Instant::now() < at)
        {
            match self.start_runtime() {
                Ok(runtime) => {
                    self.service.runtime = Some(runtime);
                    self.service.retry_at = None;
                }
                Err(error) => self.failed(ctx, error),
            }
        }
        if let Some(runtime) = self.service.runtime.as_mut() {
            let pressure = runtime.pressure();
            let result = runtime.poll_events();
            match result {
                Ok(rows) => {
                    // All authored reads precede this turn's own option/warning edits.
                    let mut ready = Vec::with_capacity(rows.len());
                    let mut rejected = false;
                    for row in rows {
                        if self.guard_input(ctx, &row) {
                            ready.push(row);
                        } else {
                            rejected = true;
                        }
                    }
                    if rejected {
                        self.set_runtime_warning(
                            ctx,
                            "Gamepad publication cancelled; source, settings or target changed",
                        );
                    }
                    let mut projected_selection = None;
                    for row in ready {
                        ctx.edits.push(golden_core::edit::Edit::CheckPreconditions {
                            preconditions: golden_core::edit::EditPreconditions::new(vec![], row.guards.to_vec())
                                .unwrap(),
                        });
                        self.clear_runtime_warning(ctx);
                        self.service.last_error = None;
                        self.service.backoff.reset();
                        let prepared = &row.prepared;
                        let catalog = &prepared.catalog;
                        if prepared.catalog_change {
                            let snapshot = ctx.tree_snapshot_arc().unwrap();
                            let node = snapshot.node(self.device.id()).unwrap();
                            // Earlier catalog rows may already normalize a blank persisted ID.
                            let expected_value = projected_selection
                                .get_or_insert_with(|| node.param_value.clone().unwrap())
                                .clone();
                            let selected_value = ParamValue::Enum(catalog.selection.clone());
                            projected_selection = Some(selected_value.clone());
                            ctx.edits.push(golden_core::edit::Edit::SetParamEnumOptions {
                                node: self.device.id(),
                                expected_uuid: node.uuid,
                                expected_value,
                                selected_value: Some(selected_value),
                                options: catalog.options.clone(),
                            });
                            self.service.catalog = Some(row.clone());
                            if gamepad_specific_device_selected(&catalog.selection) && catalog.selected.is_none() {
                                self.device.set_warning_with(
                                    ctx,
                                    Some(GAMEPAD_SELECTION_WARNING_ID),
                                    format!(
                                        "Selected gamepad '{}' is not connected.",
                                        human_gamepad_variant(&catalog.selection)
                                    ),
                                    None,
                                );
                            } else {
                                self.device.clear_warning(ctx, Some(GAMEPAD_SELECTION_WARNING_ID));
                            }
                        }
                        self.project_metadata(ctx, catalog.selected.as_ref(), catalog.devices.len());
                        self.project_values(ctx, prepared.axes, prepared.buttons);
                        if let Some(message) = &prepared.message {
                            self.last_event.set(ctx, message.clone());
                        }
                        if let Some(message) = &prepared.connected_log {
                            golden_core::logsuccess!(origin=self.id();message.clone());
                        }
                        if self.base.log_incoming_enabled() {
                            if let Some(message) = &prepared.log {
                                golden_core::log!(origin=self.id();message.clone());
                            }
                        }
                        if prepared.incoming {
                            self.base.emit_incoming_traffic(ctx);
                        }
                        prepared.emit(ctx, self.id());
                        ctx.edits.retain_payload(row);
                    }
                    if pressure != self.service.pressure {
                        self.service.pressure = pressure;
                        if pressure {
                            self.device.set_warning_with(
                                ctx,
                                Some("gamepad_pressure"),
                                "Gamepad source/callback/choice capacity is full; forwarding is paused",
                                None,
                            );
                        } else {
                            self.device.clear_warning(ctx, Some("gamepad_pressure"));
                        }
                    }
                }
                Err(error) => self.failed(ctx, error),
            }
        }
        if self
            .service
            .runtime
            .as_ref()
            .is_some_and(|r| r.has_pending() || !r.is_running())
        {
            self.request_service();
        }
        if let Some(w) = &self.service.wake {
            if let Some(at) = self.service.retry_at {
                w.later(at.saturating_duration_since(Instant::now()));
            } else {
                w.cancel_deadline();
            }
        }
    }
}
#[cfg(test)]
pub(super) mod tests;

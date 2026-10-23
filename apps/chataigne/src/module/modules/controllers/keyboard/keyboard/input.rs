use super::*;
use crate::app::module::common::worker_wake::ModuleWake;
use golden_core::edit::Edit;
use golden_io::ReconnectBackoff;
use keyboard_runtime::preparation::{Catalog, Row};
use std::time::{Duration, Instant};

pub(super) struct Service {
    #[cfg(test)]
    pub(super) factory: Option<Factory>,
    pub(super) runtime: Option<KeyboardInputRuntime>,
    pub(super) owner: RetirementPool,
    pub(super) catalog: Option<RetainedPayload<keyboard_runtime::Publication>>,
    pub(super) wake: Option<ModuleWake>,
    pub(super) retry_at: Option<Instant>,
    pressure: bool,
    backoff: ReconnectBackoff,
    last_error: Option<String>,
}
#[cfg(test)]
type Factory =
    Box<dyn Fn(&RetirementPool, golden_io::WorkerNotifier, &str) -> Result<KeyboardInputRuntime, String> + Send + Sync>;
impl Service {
    pub(super) fn new() -> Self {
        Self {
            #[cfg(test)]
            factory: None,
            runtime: None,
            owner: RetirementPool::new(NonZeroUsize::MIN),
            catalog: None,
            wake: None,
            retry_at: None,
            pressure: false,
            backoff: ReconnectBackoff::new(Duration::from_millis(250), Duration::from_secs(5)),
            last_error: None,
        }
    }
}
impl KeyboardModule {
    pub(super) fn register_input(&mut self, ctx: &ProcessCtx) -> Result<(), String> {
        let waker = match ctx.node_waker() {
            Ok(waker) => waker,
            Err(golden_core::engine::NodeWakeError::Unavailable) => return Ok(()),
            Err(error) => return Err(error.to_string()),
        };
        if !self.input.wake.as_ref().is_some_and(|wake| wake.matches(&waker)) {
            self.stop_input_backend();
            self.input.wake = Some(ModuleWake::new(waker)?);
        }
        Ok(())
    }
    pub(super) fn request_input_service(&self) {
        if let Some(wake) = &self.input.wake {
            wake.notifier.notify();
        }
    }
    pub(super) fn stop_input_backend(&mut self) {
        self.input.runtime = None;
        self.input.catalog = None;
        self.input.pressure = false;
        self.input.retry_at = None;
        if let Some(wake) = &self.input.wake {
            wake.cancel_deadline();
        }
    }
    pub(super) fn devices(&self) -> &[DiscoveredKeyboardDevice] {
        self.input
            .catalog
            .as_ref()
            .map_or(&[], |row| row.prepared.catalog.devices.as_slice())
    }
    pub(super) fn service_input(&mut self, ctx: &mut ProcessCtx) {
        if ctx.tree_snapshot_arc().is_none() {
            return;
        }
        if let Err(error) = self.register_input(ctx) {
            self.set_backend_warning(ctx, KEYBOARD_INPUT_WARNING_ID, &error);
            return;
        }
        if !self.node_data().effective_enabled || !self.receive_enabled.get() {
            if self.input.runtime.is_some() || self.input.catalog.is_some() {
                self.stop_input_backend();
                self.clear_input_values(ctx);
                self.sync_device_options(ctx);
                self.refresh_input_state(ctx);
            }
            return;
        }
        if self.device.get_ref().as_str().len() > keyboard_runtime::MAX_DEVICE_TEXT {
            self.stop_input_backend();
            self.clear_input_values(ctx);
            self.refresh_input_state(ctx);
            self.set_backend_warning(ctx, KEYBOARD_INPUT_WARNING_ID, "Keyboard selection exceeds 1024 bytes");
            return;
        }
        if self.input.runtime.is_none()
            && !self.suppress_input_backend
            && !self.input.retry_at.is_some_and(|at| Instant::now() < at)
        {
            let result = self
                .input
                .wake
                .as_ref()
                .ok_or_else(|| "Keyboard input wake is unavailable".to_string())
                .and_then(|wake| {
                    self.start_input(&self.input.owner, wake.notifier.clone(), self.device.get_ref().as_str())
                });
            match result {
                Ok(runtime) => {
                    self.input.runtime = Some(runtime);
                    self.input.retry_at = None;
                }
                Err(error) => self.input_failed(ctx, error),
            }
        }
        if let Some(runtime) = self.input.runtime.as_mut() {
            if let Err(error) = runtime.configure(self.device.get_ref().as_str()) {
                self.stop_input_backend();
                self.clear_input_values(ctx);
                self.sync_device_options(ctx);
                self.set_backend_warning(ctx, KEYBOARD_INPUT_WARNING_ID, &error);
                self.refresh_input_state(ctx);
                return;
            }
            let pressure = runtime.pressure();
            let events = runtime.poll_events();
            if pressure != self.input.pressure {
                self.input.pressure = pressure;
                if pressure {
                    self.set_backend_warning(
                        ctx,
                        "keyboard_input_pressure",
                        "Keyboard input/callback/choice lifetime capacity is full; forwarding is paused",
                    );
                } else {
                    self.clear_backend_warning(ctx, "keyboard_input_pressure");
                }
            }
            match events {
                Ok(events) => {
                    // Check authored inputs for the entire bounded turn before our
                    // own catalog edits change a parameter's authored revision.
                    let mut ready = Vec::with_capacity(events.len());
                    for publication in events {
                        if !self.guard_input(ctx, &publication) {
                            self.set_backend_warning(
                                ctx,
                                KEYBOARD_INPUT_WARNING_ID,
                                "Keyboard input publication cancelled; source, settings or target retired",
                            );
                            continue;
                        }
                        ready.push(publication);
                    }
                    for publication in ready {
                        ctx.edits.push(Edit::CheckPreconditions {
                            preconditions: golden_core::edit::EditPreconditions::new(
                                vec![],
                                publication.guards.to_vec(),
                            )
                            .unwrap(),
                        });
                        match &publication.prepared.row {
                            Row::Catalog { reset } => {
                                if *reset {
                                    self.clear_input_values(ctx);
                                }
                                self.project_input_catalog(ctx, &publication.prepared.catalog);
                                self.input.catalog = Some(publication.clone());
                                self.input.backoff.reset();
                                self.input.last_error = None;
                                self.clear_backend_warning(ctx, KEYBOARD_INPUT_WARNING_ID);
                                self.refresh_selection_warning(ctx, publication.prepared.catalog.selected.as_ref());
                                self.refresh_input_state(ctx);
                            }
                            Row::Input(frame) => {
                                self.project_key_frame(ctx, frame);
                                let output_active = self.output.is_ready() && self.send_enabled.get();
                                frame.emit(ctx, self.id(), output_active);
                                self.base.emit_incoming_traffic(ctx);
                                if self.base.log_incoming_enabled() {
                                    golden_core::log!(origin=self.id(); frame.log.clone());
                                }
                            }
                        }
                        ctx.edits.retain_payload(publication);
                    }
                }
                Err(error) => self.input_failed(ctx, error),
            }
        }
        if self
            .input
            .runtime
            .as_ref()
            .is_some_and(|runtime| runtime.has_pending() || !runtime.is_running())
        {
            self.request_input_service();
        }
        if let Some(wake) = &self.input.wake {
            if let Some(at) = self.input.retry_at {
                wake.later(at.saturating_duration_since(Instant::now()));
            } else {
                wake.cancel_deadline();
            }
        }
    }
    fn input_failed(&mut self, ctx: &mut ProcessCtx, error: String) {
        self.stop_input_backend();
        if self.input.last_error.as_ref() != Some(&error) {
            golden_core::logerror!(origin=self.id(); format!("Keyboard input stopped: {error}"));
            self.input.last_error = Some(error.clone());
        }
        self.set_backend_warning(ctx, KEYBOARD_INPUT_WARNING_ID, &error);
        self.clear_backend_warning(ctx, "keyboard_input_pressure");
        self.clear_input_values(ctx);
        self.sync_device_options(ctx);
        self.refresh_input_state(ctx);
        self.input.retry_at = Some(self.input.backoff.schedule(Instant::now()));
    }
    fn start_input(
        &self,
        owner: &RetirementPool,
        notifier: golden_io::WorkerNotifier,
        selection: &str,
    ) -> Result<KeyboardInputRuntime, String> {
        #[cfg(test)]
        if let Some(factory) = &self.input.factory {
            return factory(owner, notifier, selection);
        }
        KeyboardInputRuntime::create(owner, notifier, selection)
    }
    fn project_input_catalog(&mut self, ctx: &mut ProcessCtx, catalog: &Catalog) {
        let Some(snapshot) = ctx.tree_snapshot_arc() else {
            return;
        };
        let Some(node) = snapshot.node(self.device.id()) else {
            return;
        };
        let Some(current) = &node.param_value else {
            return;
        };
        // Normalize persisted IDs atomically with their canonical domain.
        ctx.edits.push(Edit::SetParamEnumOptions {
            node: self.device.id(),
            expected_uuid: node.uuid,
            expected_value: current.clone(),
            selected_value: Some(ParamValue::Enum(catalog.selection.clone())),
            options: catalog.options.clone(),
        });
    }
}

#[cfg(test)]
mod tests;

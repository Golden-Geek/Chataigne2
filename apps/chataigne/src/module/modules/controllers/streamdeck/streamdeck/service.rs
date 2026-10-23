use super::*;
use crate::app::module::common::worker_wake::ModuleWake;
use golden_core::edit::{EditPreconditions, NodeReadGuard};
#[cfg(any(feature = "streamdeck-hid", test))]
use golden_io::RetirementPool;
use golden_io::{ReconnectBackoff, RetainedPayload};
#[cfg(any(feature = "streamdeck-hid", test))]
use std::num::NonZeroUsize;
use std::time::{Duration, Instant};
pub(super) struct Service {
    pub(super) runtime: Option<runtime::Runtime>,
    #[cfg(any(feature = "streamdeck-hid", test))]
    pub(super) owner: RetirementPool,
    pub(super) wake: Option<ModuleWake>,
    retry_at: Option<Instant>,
    backoff: ReconnectBackoff,
    pub(super) suppressed: bool,
    pub(super) pressure: bool,
    #[cfg(test)]
    pub(super) factory: Option<Factory>,
    #[cfg(test)]
    pub(super) simulator: Option<runtime::tests::SimulatedStreamDeck>,
}
#[cfg(test)]
pub(super) type Factory = Box<
    dyn Fn(
            &RetirementPool,
            golden_io::WorkerNotifier,
            RetainedPayload<runtime::Frame>,
        ) -> Result<runtime::Runtime, String>
        + Send
        + Sync,
>;
impl Service {
    pub(super) fn new() -> Self {
        Self {
            runtime: None,
            #[cfg(any(feature = "streamdeck-hid", test))]
            owner: RetirementPool::new(NonZeroUsize::MIN),
            wake: None,
            retry_at: None,
            backoff: ReconnectBackoff::new(Duration::from_millis(250), Duration::from_secs(5)),
            suppressed: false,
            pressure: false,
            #[cfg(test)]
            factory: None,
            #[cfg(test)]
            simulator: None,
        }
    }
}
impl StreamDeckModule {
    pub(super) fn register_service(&mut self, ctx: &ProcessCtx) -> Result<(), String> {
        let waker = match ctx.node_waker() {
            Ok(waker) => waker,
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
        self.output.cancel();
        self.service.runtime = None;
        self.service.retry_at = None;
        if let Some(w) = &self.service.wake {
            w.cancel_deadline();
        }
    }
    fn start_runtime(&self, frame: RetainedPayload<runtime::Frame>) -> Result<runtime::Runtime, String> {
        let notifier = self
            .service
            .wake
            .as_ref()
            .ok_or("Stream Deck wake unavailable")?
            .notifier
            .clone();
        #[cfg(test)]
        if let Some(factory) = &self.service.factory {
            return factory(&self.service.owner, notifier, frame);
        }
        #[cfg(feature = "streamdeck-hid")]
        {
            runtime::Runtime::create(&self.service.owner, notifier, frame)
        }
        #[cfg(not(feature = "streamdeck-hid"))]
        {
            let _ = (notifier, frame);
            Err("Stream Deck hardware support is not enabled in this build".into())
        }
    }
    fn native_enabled(&self) -> bool {
        #[cfg(test)]
        if self.service.factory.is_some() {
            return true;
        }
        cfg!(feature = "streamdeck-hid")
    }
    pub(super) fn service_turn(&mut self, ctx: &mut ProcessCtx) {
        let result = self.service_inner(ctx);
        let failed = result.is_err();
        if let Err(error) = result {
            self.warning(ctx, &error);
            self.service.retry_at = Some(Instant::now() + Duration::from_millis(250));
        }
        let pressure = self.service.runtime.as_ref().is_some_and(|r| r.pressure());
        if pressure != self.service.pressure {
            self.service.pressure = pressure;
            if pressure {
                NodeHandle::new(self.id()).set_warning_with(
                    ctx,
                    Some("streamdeck_pressure"),
                    "Stream Deck publication capacity is full; input forwarding is paused",
                    None,
                );
            } else {
                NodeHandle::new(self.id()).clear_warning(ctx, Some("streamdeck_pressure"));
            }
        }
        if let Some(w) = &self.service.wake {
            if !failed
                && (self.layout.busy() && !self.layout.waiting()
                    || self
                        .service
                        .runtime
                        .as_ref()
                        .is_some_and(|r| r.pending() || !r.running()))
            {
                w.notifier.notify();
            }
            if let Some(at) = self.service.retry_at {
                w.later(at.saturating_duration_since(Instant::now()));
            } else {
                w.cancel_deadline();
            }
        }
    }
    fn service_inner(&mut self, ctx: &mut ProcessCtx) -> Result<(), String> {
        self.register_service(ctx)?;
        if self.service.retry_at.is_some_and(|at| Instant::now() >= at) {
            self.service.retry_at = None;
        }
        if !self.node_data().effective_enabled {
            return Ok(());
        }
        let snapshot = ctx
            .tree_snapshot_arc()
            .ok_or("Stream Deck service requires an engine snapshot")?;
        let owner = self.layout_owner()?;
        let notifier = self
            .service
            .wake
            .as_ref()
            .ok_or("Stream Deck wake unavailable")?
            .notifier
            .clone();
        self.layout.service(ctx, &snapshot, owner, notifier)?;
        if let Some(runtime) = &mut self.service.runtime {
            match runtime.poll() {
                Ok(rows) => self.project_rows(ctx, &snapshot, rows)?,
                Err(error) => {
                    self.stop_runtime();
                    self.base.set_connected(ctx, false);
                    self.warning(ctx, &error);
                    self.service.retry_at = Some(self.service.backoff.schedule(Instant::now()));
                    return Ok(());
                }
            }
        }
        if !self.layout.ready || self.layout.busy() {
            return Ok(());
        }
        let active = self.layout.active(&snapshot, owner.parameters)?;
        if active != self.last_active_page {
            self.last_active_page = active.clone();
            self.emit_callback(ctx, PAGE_CHANGED_CALLBACK, vec![serde_json::json!(active)]);
            self.base.emit_incoming_traffic(ctx);
        }
        if self.device.get_ref().as_str() == NO_DEVICE_VARIANT || self.service.suppressed {
            self.stop_runtime();
            self.base.set_connected(ctx, false);
            return Ok(());
        }
        if !self.native_enabled() {
            self.base.set_connected(ctx, false);
            return Ok(());
        }
        if self.output.dirty && !self.service.retry_at.is_some_and(|at| Instant::now() < at) {
            let frame = self.capture_frame(&snapshot)?;
            if let Some(runtime) = &mut self.service.runtime {
                runtime.configure(frame);
                self.output.dirty = false;
            } else {
                match self.start_runtime(frame) {
                    Ok(runtime) => {
                        self.service.runtime = Some(runtime);
                        self.service.retry_at = None;
                        self.output.dirty = false;
                    }
                    Err(error) => {
                        self.warning(ctx, &error);
                        self.service.retry_at = Some(self.service.backoff.schedule(Instant::now()));
                    }
                }
            }
        }
        Ok(())
    }
    fn project_rows(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        rows: Vec<RetainedPayload<runtime::Publication>>,
    ) -> Result<(), String> {
        let node = snapshot
            .node(self.device.id())
            .ok_or("Stream Deck device parameter was removed")?;
        let Some(ParamValue::Enum(authored)) = &node.param_value else {
            return Err("Stream Deck device must be an enum".into());
        };
        if authored.len() > runtime::MAX_SERIAL {
            return Err("Stream Deck serial exceeds 256 bytes".into());
        }
        let mut accepted = Vec::with_capacity(rows.len());
        for row in rows {
            if !row.current() || row.selection != output::selection(authored) {
                continue;
            }
            if let runtime::Event::Input(input) = &row.event {
                if !input.route.matches(snapshot) {
                    continue;
                }
                input.route.fence(ctx);
            }
            accepted.push(row);
        }
        if accepted.is_empty() {
            return Ok(());
        }
        // Read preconditions precede our own choice/warning edits. In particular a
        // blank -> Auto normalization cannot reject the next accepted status row.
        let connection = snapshot
            .find_child_by_decl_id(self.id(), "connection")
            .ok_or("Stream Deck connection folder was removed")?;
        if node.parent != Some(connection)
            || snapshot.find_child_by_decl_id(connection, "device") != Some(self.device.id())
        {
            return Ok(());
        }
        let connected = self
            .base
            .connection_status_parameter()
            .ok_or("Stream Deck connection status was removed")?;
        if snapshot.find_child_by_decl_id(connection, "connected") != Some(connected) {
            return Ok(());
        }
        let reads = vec![
            NodeReadGuard::capture(snapshot, self.device.id()).ok_or("Stream Deck device unavailable")?,
            NodeReadGuard::capture_identity(snapshot, self.id()).ok_or("Stream Deck module unavailable")?,
            NodeReadGuard::capture_structure(snapshot, connection).ok_or("Stream Deck connection path unavailable")?,
            NodeReadGuard::capture_structure(snapshot, connected).ok_or("Stream Deck connection status unavailable")?,
        ];
        ctx.edits.push(Edit::CheckPreconditions {
            preconditions: EditPreconditions::new(reads, vec![]).unwrap(),
        });
        let mut projected = ParamValue::Enum(authored.clone());
        let mut options = node
            .param_constraints
            .as_ref()
            .map(|c| c.enum_options.clone())
            .unwrap_or_default();
        for row in accepted {
            ctx.edits.push(Edit::CheckPreconditions {
                preconditions: EditPreconditions::new(vec![], row.guards.clone()).unwrap(),
            });
            match &row.event {
                runtime::Event::Input(input) => self.project_input(ctx, input),
                runtime::Event::Status(status) => {
                    let next = ParamValue::Enum(status.selection.clone());
                    if options != status.options || projected != next {
                        ctx.edits.push(Edit::SetParamEnumOptions {
                            node: self.device.id(),
                            expected_uuid: node.uuid,
                            expected_value: projected.clone(),
                            selected_value: Some(next.clone()),
                            options: status.options.clone(),
                        });
                        projected = next;
                        options = status.options.clone();
                    }
                    self.base.set_connected(ctx, status.connected);
                    if let Some(error) = &status.message {
                        self.warning(ctx, error);
                    } else {
                        NodeHandle::new(self.id()).clear_warning(ctx, Some("streamdeck_runtime"));
                        if status.connected {
                            self.service.backoff.reset();
                        }
                    }
                }
            }
            ctx.edits.retain_payload(row);
        }
        Ok(())
    }
}

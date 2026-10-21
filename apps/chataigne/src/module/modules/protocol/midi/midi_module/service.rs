use super::*;
use crate::app::module::common::worker_wake::ModuleWake;
use std::time::Instant;

pub(super) const RETRY_DELAY: Duration = Duration::from_secs(1);

#[derive(Default)]
pub(super) struct ServiceState {
    pub wake: Option<ModuleWake>,
    pub discovery_retry: Option<Instant>,
    pub input_retry: Option<Instant>,
    pub output_retry: Option<Instant>,
}
pub(super) fn due(deadline: Option<Instant>) -> bool {
    deadline.is_none_or(|at| Instant::now() >= at)
}
impl MidiModule {
    pub(super) fn request_service(&self, ctx: &ProcessCtx) {
        if let Ok(waker) = ctx.node_waker() {
            let _ = waker.wake();
        } else if let Some(wake) = &self.service.wake {
            wake.notifier.notify();
        }
    }
    pub(super) fn register_service(&mut self, ctx: &mut ProcessCtx) -> Result<(), String> {
        let source = ctx.node_waker().map_err(|error| error.to_string())?;
        if !self.service.wake.as_ref().is_some_and(|wake| wake.matches(&source)) {
            self.stop_input();
            self.stop_output();
            self.discovery = None;
            self.pending_catalog = None;
            self.service = ServiceState::default();
            self.service.wake = Some(ModuleWake::new(source)?);
            self.input_dirty = true;
            self.output_dirty = true;
            self.refresh_data_capabilities(ctx);
            ctx.clear_node_warning(self.id(), Some("midi_service"));
        }
        Ok(())
    }
    pub(super) fn service(&mut self, ctx: &mut ProcessCtx) {
        if let Some(wake) = &self.service.wake {
            wake.cancel_deadline();
        }
        let snapshot = ctx.tree_snapshot_arc();
        self.service_structure(ctx, snapshot.as_deref());
        if !self.node_data().effective_enabled {
            return;
        }
        if let Err(error) = self.register_service(ctx) {
            ctx.set_node_warning_with(self.id(), Some("midi_service"), error, None);
            return;
        }
        if self.discovery.is_none() && due(self.service.discovery_retry) {
            self.refresh_port_options(ctx);
        } else {
            self.service_catalog(ctx);
        }
        if self.output.as_ref().is_some_and(MidiOutputHandle::has_pending)
            || self.retiring_output.as_ref().is_some_and(MidiOutputHandle::has_pending)
        {
            self.drain_output_events(ctx);
        }
        if self.input.as_ref().is_some_and(MidiInputHandle::has_pending) {
            self.drain_input_events(ctx);
        }
        self.process_pending_incoming(ctx, snapshot.as_deref());
        if self.input_dirty && due(self.service.input_retry) {
            self.refresh_input(ctx);
        }
        if self.output_dirty && self.retiring_output.is_none() && due(self.service.output_retry) {
            self.refresh_output(ctx);
        }
        self.refresh_connected(ctx);
        self.refresh_transport_activity(ctx);
        self.arm_service(ctx);
    }
    pub(super) fn service_requires_snapshot(&self) -> bool {
        self.structure_work.requires_snapshot()
            || self.node_data().effective_enabled
                && (self.catalog_requires_snapshot()
                    || !self.structure_work.busy()
                        && (!self.pending_incoming_messages.is_empty()
                            || self
                                .incoming
                                .as_ref()
                                .is_some_and(input_delivery::InputDelivery::requires_snapshot)))
    }
    pub(super) fn input_can_drain(&self) -> bool {
        !self.structure_work.busy()
            && self.incoming.is_none()
            && self.pending_incoming_messages.is_empty()
            && self.incoming_callbacks.len() < 16
    }
    fn arm_service(&self, ctx: &ProcessCtx) {
        let immediate = self.structure_work.busy()
            || self.incoming.is_some()
            || !self.incoming_callbacks.is_empty()
            || !self.pending_incoming_messages.is_empty()
            || self.pending_catalog.is_some()
            || self.discovery.as_ref().is_some_and(MidiDiscovery::has_pending)
            || self.output.as_ref().is_some_and(MidiOutputHandle::has_pending)
            || self.retiring_output.as_ref().is_some_and(MidiOutputHandle::has_pending)
            || self.input_can_drain() && self.input.as_ref().is_some_and(MidiInputHandle::has_pending)
            || self.input_dirty && due(self.service.input_retry)
            || self.output_dirty && self.retiring_output.is_none() && due(self.service.output_retry);
        if immediate {
            self.request_service(ctx);
        }
        let now = Instant::now();
        let retry = [
            self.discovery
                .is_none()
                .then_some(self.service.discovery_retry)
                .flatten(),
            self.input_dirty.then_some(self.service.input_retry).flatten(),
            (self.output_dirty && self.retiring_output.is_none())
                .then_some(self.service.output_retry)
                .flatten(),
        ]
        .into_iter()
        .flatten()
        .map(|at| at.saturating_duration_since(now));
        let activity = self
            .activity_deadline()
            .map(|at| at.saturating_sub(ctx.runtime_elapsed));
        if let (Some(wake), Some(delay)) = (&self.service.wake, retry.chain(activity).min()) {
            wake.later(delay);
        }
    }
}

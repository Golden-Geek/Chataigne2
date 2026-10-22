use super::*;
use crate::app::module::common::worker_wake::ModuleWake;
use golden_io::ReconnectBackoff;
use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};
use transport::{CommandReceipt, CommandStatus};

pub(super) struct Service {
    pub(super) wake: Option<ModuleWake>,
    pub(super) pending: VecDeque<CommandReceipt>,
    pub(super) retry_at: Option<Instant>,
    backoff: ReconnectBackoff,
    native_slot: golden_io::RetirementPool,
    pub(super) admission: golden_io::RetirementPool,
    pub(super) outgoing: VecDeque<golden_io::RetainedPayload<transport::Source>>,
    pub(super) output_retry_at: Option<Instant>,
}
impl Service {
    pub(super) fn new() -> Self {
        Self {
            wake: None,
            pending: VecDeque::new(),
            retry_at: None,
            backoff: ReconnectBackoff::new(Duration::from_millis(250), Duration::from_secs(5)),
            native_slot: golden_io::RetirementPool::new(std::num::NonZeroUsize::MIN),
            admission: golden_io::RetirementPool::new(std::num::NonZeroUsize::new(64).unwrap()),
            outgoing: VecDeque::new(),
            output_retry_at: None,
        }
    }
}
impl ButtplugModule {
    pub(super) fn register_service(&mut self, ctx: &ProcessCtx) -> Result<(), String> {
        let source = match ctx.node_waker() {
            Ok(source) => source,
            Err(golden_core::engine::NodeWakeError::Unavailable) => return Ok(()),
            Err(error) => return Err(error.to_string()),
        };
        if !self.service.wake.as_ref().is_some_and(|wake| wake.matches(&source)) {
            self.stop_transport();
            self.last_transport_config = None;
            self.transport_dirty = true;
            self.service.wake = Some(ModuleWake::new(source)?);
        }
        Ok(())
    }
    pub(super) fn request_service(&self) {
        if let Some(wake) = &self.service.wake {
            wake.notifier.notify();
        }
    }
    fn schedule_retry(&mut self) {
        let at = self.service.backoff.schedule(Instant::now());
        self.service.retry_at = Some(at);
        if let Some(wake) = &self.service.wake {
            wake.later(at.saturating_duration_since(Instant::now()));
        }
    }
    pub(super) fn service_io(&mut self, ctx: &mut ProcessCtx) {
        for _ in 0..self.service.pending.len().min(16) {
            let receipt = self.service.pending.pop_front().unwrap();
            let Some(snapshot) = ctx.tree_snapshot_arc() else {
                self.service.pending.push_back(receipt);
                continue;
            };
            if let Some(result) = receipt.take() {
                if let Some(guard) = golden_core::edit::NodeReadGuard::capture_identity(&snapshot, self.id()) {
                    ctx.edits.push(golden_core::edit::Edit::CheckPreconditions {
                        preconditions: golden_core::edit::EditPreconditions::new(vec![guard], vec![]).unwrap(),
                    });
                }
                if receipt.attempted() {
                    self.base.emit_outgoing_traffic(ctx);
                }
                match result {
                    CommandStatus::Completed(Ok(message)) => {
                        if self.base.log_outgoing_enabled() {
                            golden_core::log!(origin=self.id(); format!("Buttplug {message}."));
                        }
                    }
                    CommandStatus::Completed(Err(error)) => {
                        self.set_target_warning(ctx, &error);
                        logerror!(error);
                    }
                    CommandStatus::NotSent => {
                        logerror!("Buttplug request cancelled; it was not sent");
                    }
                    CommandStatus::Unconfirmed => {
                        logerror!("Buttplug request interrupted; its result is unconfirmed");
                    }
                }
                ctx.edits.retain_payload(receipt.source());
            } else {
                self.service.pending.push_back(receipt);
            }
        }
        if self.service.pending.iter().any(CommandReceipt::complete) {
            self.request_service();
        }
        if !self.node_data().effective_enabled {
            return;
        }
        let Some(snapshot) = ctx.tree_snapshot_arc() else {
            return;
        };
        self.drain_transport_events(ctx);
        if self.service.retry_at.is_some_and(|at| Instant::now() >= at) {
            self.service.retry_at = None;
            self.transport_dirty = true;
        }
        if self.transport_dirty && self.service.retry_at.is_none() {
            self.refresh_data_capabilities(ctx);
            self.refresh_transport(ctx, &snapshot);
        }
        self.submit_outputs(ctx);
        if self
            .transport
            .as_deref()
            .is_some_and(ButtplugTransportHandle::has_pending)
        {
            self.request_service();
        }
        if let (Some(wake), Some(at)) = (
            &self.service.wake,
            self.service
                .retry_at
                .into_iter()
                .chain(self.service.output_retry_at)
                .min(),
        ) {
            wake.later(at.saturating_duration_since(Instant::now()));
        }
    }
    fn drain_transport_events(&mut self, ctx: &mut ProcessCtx) {
        for _ in 0..16 {
            let Some(transport) = &self.transport else {
                break;
            };
            if !transport.is_running() {
                self.stop_transport();
                self.last_transport_config = None;
                self.transport_dirty = true;
                self.base.set_connected(ctx, false);
                self.set_target_warning(ctx, "Buttplug worker stopped unexpectedly; recovering");
                self.schedule_retry();
                break;
            }
            let mut events = Vec::with_capacity(1);
            transport.drain_events(&mut events);
            let Some(event) = events.pop() else {
                break;
            };
            if matches!(
                &*event,
                ButtplugWorkerEvent::Status(ButtplugConnectionStatus::Connected { .. })
            ) {
                self.service.backoff.reset();
            }
            self.handle_worker_event(ctx, (*event).clone());
        }
    }
    fn refresh_transport(&mut self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot) {
        self.transport_dirty = false;

        if !self.module_enabled(snapshot) {
            self.stop_transport();
            self.last_transport_config = None;
            self.clear_target_warning(ctx);
            self.base.set_connected(ctx, false);
            self.set_bool_param(ctx, ButtplugBoolParam::Scanning, false);
            self.sync_device_options(ctx);
            self.refresh_device_values(ctx);
            return;
        }

        let config = match self.transport_config() {
            Ok(config) => config,
            Err(error) => {
                logerror!("Invalid Buttplug module configuration: {}", error);
                self.stop_transport();
                self.last_transport_config = None;
                self.set_target_warning(ctx, error.as_str());
                self.base.set_connected(ctx, false);
                self.sync_device_options(ctx);
                self.refresh_device_values(ctx);
                return;
            }
        };

        if self.transport.is_some() && self.last_transport_config.as_deref() == Some(&config) {
            return;
        }

        self.stop_transport();

        let Some(wake) = &self.service.wake else {
            return;
        };
        match ButtplugTransportHandle::spawn(config.clone(), wake.notifier.clone(), &self.service.native_slot) {
            Ok(handle) => {
                self.service.retry_at = None;
                self.transport = Some(Box::new(handle));
                self.last_transport_config = Some(Box::new(config));
                self.clear_target_warning(ctx);
                self.base.set_connected(ctx, false);
            }
            Err(error) => {
                logerror!("Failed to start Buttplug transport: {}", error);
                self.transport = None;
                self.last_transport_config = None;
                self.set_target_warning(ctx, error.as_str());
                self.base.set_connected(ctx, false);
                self.sync_device_options(ctx);
                self.refresh_device_values(ctx);
                self.schedule_retry();
            }
        }
    }
}

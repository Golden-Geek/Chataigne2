use super::*;
use crate::app::module::common::worker_wake::ModuleWake;
use golden_io::{ReconnectBackoff, RetirementPool};
use std::{collections::VecDeque, num::NonZeroUsize, time::Instant};

pub(super) struct Service {
    pub(super) wake: Option<ModuleWake>,
    pub(super) pending: VecDeque<runtime::Receipt>,
    pub(super) outgoing: VecDeque<golden_io::RetainedPayload<runtime::Source>>,
    pub(super) admission: RetirementPool,
    pub(super) ingress: RetirementPool,
    pub(super) output_retry_at: Option<Instant>,
    pub(super) owner: RetirementPool,
    pub(super) retry_at: Option<Instant>,
    backoff: ReconnectBackoff,
    pub(super) last_turn: Instant,
    pub(super) activity_until: Option<Instant>,
    input_pressure: bool,
}
impl Service {
    pub(super) fn connected_observation(&mut self) {
        self.backoff.reset();
    }
    pub(super) fn new() -> Self {
        Self {
            wake: None,
            pending: VecDeque::new(),
            outgoing: VecDeque::new(),
            admission: RetirementPool::new(NonZeroUsize::new(64).unwrap()),
            ingress: RetirementPool::new(NonZeroUsize::new(64).unwrap()),
            output_retry_at: None,
            owner: RetirementPool::new(NonZeroUsize::MIN),
            retry_at: None,
            backoff: ReconnectBackoff::new(Duration::from_millis(250), Duration::from_secs(5)),
            last_turn: Instant::now(),
            activity_until: None,
            input_pressure: false,
        }
    }
}
impl JoyConModule {
    pub(super) fn register_service(&mut self, ctx: &ProcessCtx) -> Result<(), String> {
        let waker = match ctx.node_waker() {
            Ok(waker) => waker,
            Err(golden_core::engine::NodeWakeError::Unavailable) => return Ok(()),
            Err(error) => return Err(error.to_string()),
        };
        if !self.service.wake.as_ref().is_some_and(|wake| wake.matches(&waker)) {
            self.stop_runtime();
            self.runtime_dirty = true;
            self.service.wake = Some(ModuleWake::new(waker)?);
        }
        Ok(())
    }
    pub(super) fn request_service(&self) {
        if let Some(wake) = &self.service.wake {
            wake.notifier.notify();
        }
    }
    pub(super) fn schedule_retry(&mut self) {
        self.service.retry_at = Some(self.service.backoff.schedule(Instant::now()));
    }
    pub(super) fn service_io(&mut self, ctx: &mut ProcessCtx) {
        if ctx.tree_snapshot_arc().is_none() {
            return;
        }
        self.guard_settings(ctx);
        let now = Instant::now();
        let elapsed = now.saturating_duration_since(self.service.last_turn);
        self.service.last_turn = now;
        self.processing_elapsed = self
            .processing_elapsed
            .saturating_add(elapsed)
            .min(processing_interval_from_fps_cap(self.processing_fps_cap.get()));
        self.incoming_log_elapsed = self.incoming_log_elapsed.saturating_add(elapsed);
        self.runtime_contact_elapsed = self.runtime_contact_elapsed.saturating_add(elapsed);
        self.project_results(ctx);
        self.ensure_runtime(ctx);
        self.configure_projection(ctx);
        let pressure = self.runtime.as_ref().is_some_and(|runtime| runtime.pressure());
        if pressure != self.service.input_pressure && self.guard_warning(ctx) {
            self.service.input_pressure = pressure;
            if pressure {
                self.motion_data_mode.set_warning_with(
                    ctx,
                    Some("joycon_input_pressure"),
                    "Joy-Con input lifetime capacity is full; native input is paused",
                    None,
                );
            } else {
                self.motion_data_mode.clear_warning(ctx, Some("joycon_input_pressure"));
            }
        }
        self.drain_runtime_events(ctx);
        self.apply_pending_state_if_due(ctx, false);
        if self.node_data().effective_enabled {
            self.submit_outputs(ctx);
        }
        self.update_activity_indicator(ctx);
        self.restart_stalled_runtime_if_needed(ctx);
        if self.runtime.as_ref().is_some_and(|runtime| runtime.has_pending())
            || self.service.pending.iter().any(runtime::Receipt::complete)
        {
            self.request_service();
        }
        self.arm_deadline();
    }
    fn arm_deadline(&self) {
        let Some(wake) = &self.service.wake else {
            return;
        };
        let now = Instant::now();
        let mut deadlines = Vec::with_capacity(4);
        if self.node_data().effective_enabled {
            if let Some(at) = self.service.retry_at {
                deadlines.push(at);
            }
            if let Some(at) = self.service.output_retry_at {
                deadlines.push(at);
            }
            if self.pending_state.is_some() {
                deadlines.push(
                    now + processing_interval_from_fps_cap(self.processing_fps_cap.get())
                        .saturating_sub(self.processing_elapsed),
                );
            }
            if self.runtime.is_some()
                && runtime_expected_to_be_live(
                    self.pending_state.as_ref().and_then(|value| value.state()),
                    &self.last_state,
                )
            {
                deadlines.push(now + JOYCON_RUNTIME_STALL_TIMEOUT.saturating_sub(self.runtime_contact_elapsed));
            }
            if let Some(at) = self.service.activity_until {
                deadlines.push(at);
            }
        }
        if let Some(at) = deadlines.into_iter().min() {
            wake.later(at.saturating_duration_since(now));
        } else {
            wake.cancel_deadline();
        }
    }
    pub(super) fn has_pending_results(&self) -> bool {
        !self.service.pending.is_empty()
    }
    pub(super) fn clear_input_pressure(&mut self, ctx: &mut ProcessCtx) {
        if self.service.input_pressure && self.guard_warning(ctx) {
            self.motion_data_mode.clear_warning(ctx, Some("joycon_input_pressure"));
            self.service.input_pressure = false;
        }
    }
    fn project_results(&mut self, ctx: &mut ProcessCtx) {
        for _ in 0..self.service.pending.len().min(16) {
            let receipt = self.service.pending.pop_front().unwrap();
            let Some(result) = receipt.take() else {
                self.service.pending.push_back(receipt);
                continue;
            };
            if !self.guard_result(ctx, &[]) {
                golden_core::logerror!(format!(
                    "Joy-Con completion target unavailable; disposition: {result:?}"
                ));
                ctx.edits.retain_payload(receipt.source());
                continue;
            }
            match result {
                runtime::Outcome::Completed(Ok(message)) => self.set_last_event(ctx, message),
                runtime::Outcome::Completed(Err(error)) => {
                    self.set_runtime_warning(ctx, &error);
                    self.set_last_event(ctx, format!("Command failed: {error}"));
                }
                runtime::Outcome::NotSent => self.set_last_event(ctx, "Joy-Con command cancelled; not sent".into()),
                runtime::Outcome::Unconfirmed => {
                    self.set_last_event(ctx, "Joy-Con command interrupted; result unconfirmed".into())
                }
            }
            if receipt.attempted() {
                self.base.emit_outgoing_traffic(ctx);
            }
            ctx.edits.retain_payload(receipt.source());
        }
    }
    pub(super) fn ensure_runtime(&mut self, ctx: &mut ProcessCtx) {
        if !self.node_data().effective_enabled {
            self.stop_runtime();
            self.pending_state = None;
            self.clear_runtime_warning(ctx);
            self.clear_input_pressure(ctx);
            self.apply_runtime_state(ctx, JoyConRuntimeState::disconnected());
            self.set_bool_handle(ctx, JoyConBoolParam::Activity, false);
            return;
        }
        if self.runtime.is_some() && !self.runtime_dirty {
            return;
        }
        if self.service.retry_at.is_some_and(|at| Instant::now() < at) {
            return;
        }
        let Some(notifier) = self.service.wake.as_ref().map(|wake| wake.notifier.clone()) else {
            return;
        };
        self.stop_runtime();
        match JoyConRuntimeHandle::spawn(notifier, &self.service.owner) {
            Ok(handle) => {
                self.runtime = Some(Box::new(handle));
                self.runtime_dirty = false;
                self.service.retry_at = None;
                self.runtime_contact_elapsed = Duration::ZERO;
            }
            Err(error) => {
                self.runtime_dirty = true;
                self.set_runtime_warning(ctx, &error);
                self.base.set_connected(ctx, false);
                self.schedule_retry();
            }
        }
    }
    pub(super) fn stop_runtime(&mut self) {
        self.runtime.take();
        if !self.service.outgoing.is_empty() {
            golden_core::logerror!(origin=self.id(); format!("Joy-Con retirement cancelled {} captured requests; not sent", self.service.outgoing.len()));
        }
        self.service.outgoing.clear();
        self.service.output_retry_at = None;
        self.service.retry_at = None;
    }
    pub(super) fn note_incoming_state(
        &mut self,
        ctx: &mut ProcessCtx,
        previous_state: &JoyConRuntimeState,
        state: &JoyConRuntimeState,
        motion_mode: JoyConMotionDataMode,
    ) {
        if runtime_state_has_visible_input_change(previous_state, state, self.stick_dead_zone.get(), motion_mode) {
            self.base.emit_incoming_traffic(ctx);
            self.service.activity_until = Some(Instant::now() + JOYCON_ACTIVITY_HOLD_DURATION);
            self.set_bool_handle(ctx, JoyConBoolParam::Activity, true);

            if self.base.log_incoming_enabled() && self.incoming_log_elapsed >= JOYCON_INCOMING_LOG_INTERVAL {
                self.incoming_log_elapsed = Duration::ZERO;
                golden_core::log!(
                    origin = self.id();
                    summarize_runtime_state_for_log(state, self.stick_dead_zone.get(), motion_mode)
                );
            }
        }

        if connection_state_changed(&self.last_state, state) {
            self.log_connection_transition(ctx, state);
        }
    }

    fn log_connection_transition(&mut self, ctx: &mut ProcessCtx, state: &JoyConRuntimeState) {
        for (label, was_connected, is_connected) in [
            ("Left Controller", self.last_state.left.connected, state.left.connected),
            (
                "Right Controller",
                self.last_state.right.connected,
                state.right.connected,
            ),
        ] {
            if was_connected == is_connected {
                continue;
            }

            let message = if is_connected {
                format!("{label} connected")
            } else {
                format!("{label} disconnected")
            };
            self.set_last_event(ctx, message.clone());
            if self.base.log_incoming_enabled() {
                golden_core::log!(origin = self.id(); message);
            }
        }
    }

    fn update_activity_indicator(&mut self, ctx: &mut ProcessCtx) {
        if !self.activity.get() {
            self.service.activity_until = None;
            return;
        }
        if self.service.activity_until.is_some_and(|at| Instant::now() >= at) {
            if self.guard_activity(ctx) {
                self.set_bool_handle(ctx, JoyConBoolParam::Activity, false);
            }
            self.service.activity_until = Some(Instant::now() + Duration::from_millis(250));
        }
    }

    fn restart_stalled_runtime_if_needed(&mut self, ctx: &mut ProcessCtx) {
        if self.runtime.is_none() || self.runtime_dirty {
            return;
        }
        if self.runtime_contact_elapsed < JOYCON_RUNTIME_STALL_TIMEOUT {
            return;
        }
        if !runtime_expected_to_be_live(
            self.pending_state.as_ref().and_then(|value| value.state()),
            &self.last_state,
        ) {
            return;
        }

        self.stop_runtime();
        self.runtime_dirty = true;
        self.pending_state = None;
        self.processing_elapsed = Duration::ZERO;
        self.service.activity_until = None;
        self.incoming_log_elapsed = JOYCON_INCOMING_LOG_INTERVAL;
        self.runtime_contact_elapsed = Duration::ZERO;
        self.schedule_retry();
        self.set_runtime_warning(ctx, "Joy-Con runtime stalled; restarting");
        self.set_last_event(ctx, "Joy-Con runtime stalled, restarting".to_string());
        self.apply_runtime_state(ctx, JoyConRuntimeState::disconnected());
        self.set_bool_handle(ctx, JoyConBoolParam::Activity, false);
    }
}

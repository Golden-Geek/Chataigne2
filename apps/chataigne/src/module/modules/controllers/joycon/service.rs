use super::*;
use crate::app::module::common::worker_wake::ModuleWake;
use golden_io::{ReconnectBackoff, RetirementPool};
use std::{collections::VecDeque, num::NonZeroUsize, time::Instant};

pub(super) struct Service {
    pub(super) wake: Option<ModuleWake>,
    pending: VecDeque<runtime::Receipt>,
    pub(super) owner: RetirementPool,
    pub(super) retry_at: Option<Instant>,
    backoff: ReconnectBackoff,
    pub(super) last_turn: Instant,
    pub(super) activity_until: Option<Instant>,
    input_pressure: bool,
}
impl Service {
    pub(super) fn new() -> Self {
        Self {
            wake: None,
            pending: VecDeque::new(),
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
    fn schedule_retry(&mut self) {
        self.service.retry_at = Some(self.service.backoff.schedule(Instant::now()));
    }
    pub(super) fn service_io(&mut self, ctx: &mut ProcessCtx) {
        if ctx.tree_snapshot_arc().is_none() {
            return;
        }
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
        let pressure = self.runtime.as_ref().is_some_and(|runtime| runtime.pressure());
        if pressure != self.service.input_pressure {
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
        let force = self.drain_runtime_events(ctx);
        self.apply_pending_state_if_due(ctx, force);
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
            if self.pending_state.is_some() {
                deadlines.push(
                    now + processing_interval_from_fps_cap(self.processing_fps_cap.get())
                        .saturating_sub(self.processing_elapsed),
                );
            }
            if self.runtime.is_some() && runtime_expected_to_be_live(self.pending_state.as_ref(), &self.last_state) {
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
    pub(super) fn enqueue_receipt(&mut self, receipt: runtime::Receipt) {
        self.service.pending.push_back(receipt);
    }
    pub(super) fn can_accept_command(&self) -> bool {
        self.service.pending.len() < 64
    }
    pub(super) fn has_pending_results(&self) -> bool {
        !self.service.pending.is_empty()
    }
    fn project_results(&mut self, ctx: &mut ProcessCtx) {
        for _ in 0..self.service.pending.len().min(16) {
            let receipt = self.service.pending.pop_front().unwrap();
            let Some(result) = receipt.take() else {
                self.service.pending.push_back(receipt);
                continue;
            };
            let snapshot = ctx.tree_snapshot_arc().unwrap();
            if let Some(guard) = golden_core::edit::NodeReadGuard::capture_identity(&snapshot, self.id()) {
                ctx.edits.push(golden_core::edit::Edit::CheckPreconditions {
                    preconditions: golden_core::edit::EditPreconditions::new(vec![guard], vec![]).unwrap(),
                });
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
            ctx.edits.retain_payload(receipt.source());
        }
    }
    pub(super) fn ensure_runtime(&mut self, ctx: &mut ProcessCtx) {
        if !self.node_data().effective_enabled {
            self.stop_runtime();
            self.pending_state = None;
            self.clear_runtime_warning(ctx);
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
        self.service.retry_at = None;
    }
    pub(super) fn drain_runtime_events(&mut self, ctx: &mut ProcessCtx) -> bool {
        for _ in 0..2 {
            let Some(runtime) = &self.runtime else {
                break;
            };
            match runtime.try_recv() {
                Ok(publication) => {
                    if !publication.guards.iter().all(golden_io::RevisionGuard::is_current) {
                        continue;
                    }
                    let snapshot = ctx.tree_snapshot_arc().unwrap();
                    let Some(read) = golden_core::edit::NodeReadGuard::capture_identity(&snapshot, self.id()) else {
                        continue;
                    };
                    ctx.edits.push(golden_core::edit::Edit::CheckPreconditions {
                        preconditions: golden_core::edit::EditPreconditions::new(
                            vec![read],
                            publication.guards.to_vec(),
                        )
                        .unwrap(),
                    });
                    self.runtime_contact_elapsed = Duration::ZERO;
                    match &publication.event {
                        JoyConWorkerEvent::Heartbeat => {}
                        JoyConWorkerEvent::State(state) => {
                            self.service.backoff.reset();
                            self.clear_runtime_warning(ctx);
                            let reference = self.pending_state.as_ref().unwrap_or(&self.last_state);
                            let discrete = connection_state_changed(reference, state)
                                || JoyConSide::ALL
                                    .iter()
                                    .any(|side| controller_buttons_changed(reference.side(*side), state.side(*side)));
                            self.pending_state = Some((**state).clone());
                            if discrete {
                                self.apply_pending_state_if_due(ctx, true);
                            }
                        }
                        JoyConWorkerEvent::Error(error) => {
                            self.set_runtime_warning(ctx, error);
                            self.set_last_event(ctx, error.clone());
                        }
                    }
                    ctx.edits.retain_payload(publication);
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.stop_runtime();
                    self.runtime_dirty = true;
                    self.pending_state = None;
                    self.apply_runtime_state(ctx, JoyConRuntimeState::disconnected());
                    self.set_runtime_warning(ctx, "Joy-Con runtime stopped unexpectedly");
                    self.schedule_retry();
                    break;
                }
            }
        }
        false
    }

    fn apply_pending_state_if_due(&mut self, ctx: &mut ProcessCtx, force_apply: bool) {
        let Some(state) = self.pending_state.clone() else {
            return;
        };

        if !force_apply {
            let processing_interval = processing_interval_from_fps_cap(self.processing_fps_cap.get());
            if self.processing_elapsed < processing_interval {
                return;
            }
            self.processing_elapsed = self.processing_elapsed.saturating_sub(processing_interval);
        } else {
            self.processing_elapsed = Duration::ZERO;
        }

        self.pending_state = None;
        let previous_state = self.last_state.clone();
        let motion_mode = self.motion_data_mode();
        self.note_incoming_state(ctx, &previous_state, &state, motion_mode);
        self.apply_runtime_state(ctx, state);
        self.emit_runtime_callbacks(ctx, &previous_state, motion_mode);
    }

    fn note_incoming_state(
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
        if self.service.activity_until.is_some_and(|at| Instant::now() >= at) {
            self.service.activity_until = None;
            self.set_bool_handle(ctx, JoyConBoolParam::Activity, false);
        }
    }

    fn restart_stalled_runtime_if_needed(&mut self, ctx: &mut ProcessCtx) {
        if self.runtime.is_none() || self.runtime_dirty {
            return;
        }
        if self.runtime_contact_elapsed < JOYCON_RUNTIME_STALL_TIMEOUT {
            return;
        }
        if !runtime_expected_to_be_live(self.pending_state.as_ref(), &self.last_state) {
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

use super::*;
use golden_core::{
    edit::{Edit, EditPreconditions, NodeReadGuard},
    process_ctx::ProcessTreeSnapshot,
};
use golden_io::RevisionGuard;

fn collect(
    snapshot: &ProcessTreeSnapshot,
    root: NodeId,
    path: &str,
    leaves: &[(Option<NodeId>, &str)],
    ids: &mut Vec<NodeId>,
) -> bool {
    for (id, name) in leaves {
        let Some(id) = *id else {
            return false;
        };
        let Some(node) = snapshot.node(id) else {
            return false;
        };
        if node.decl_id.rsplit('/').next() != Some(*name) || node.param_value.is_none() {
            return false;
        }
        ids.push(id);
        let mut parent = node.parent;
        for part in path.rsplit('/') {
            let Some(id) = parent else {
                return false;
            };
            let Some(node) = snapshot.node(id) else {
                return false;
            };
            if node.decl_id.rsplit('/').next() != Some(part) {
                return false;
            }
            ids.push(id);
            parent = node.parent;
        }
        if parent != Some(root) {
            return false;
        }
    }
    true
}
impl JoyConModule {
    fn finish_guards(&self, ctx: &mut ProcessCtx, mut ids: Vec<NodeId>, native: &[RevisionGuard]) -> bool {
        let Some(snapshot) = ctx.tree_snapshot_arc() else {
            return false;
        };
        ids.sort_unstable_by_key(|id| id.0);
        ids.dedup();
        let Some(root) = NodeReadGuard::capture_identity(&snapshot, self.id()) else {
            return false;
        };
        let mut reads = vec![root];
        for id in ids {
            let Some(guard) = NodeReadGuard::capture_structure(&snapshot, id) else {
                return false;
            };
            reads.push(guard);
        }
        // Every bounded fence precedes every value write and callback in this projection.
        for chunk in reads.chunks(32) {
            ctx.edits.push(Edit::CheckPreconditions {
                preconditions: EditPreconditions::new(chunk.to_vec(), native.to_vec()).unwrap(),
            });
        }
        true
    }
    pub(super) fn guard_settings(&self, ctx: &mut ProcessCtx) {
        if !self.motion_data_mode.is_bound() || !self.stick_dead_zone.is_bound() {
            return;
        }
        let Some(snapshot) = ctx.tree_snapshot_arc() else {
            return;
        };
        let reads: Option<Vec<_>> = [self.motion_data_mode.id(), self.stick_dead_zone.id()]
            .iter()
            .map(|id| NodeReadGuard::capture(&snapshot, *id))
            .collect();
        if let Some(reads) = reads {
            ctx.edits.push(Edit::CheckPreconditions {
                preconditions: EditPreconditions::new(reads, vec![]).unwrap(),
            });
        }
    }
    pub(super) fn guard_warning(&self, ctx: &mut ProcessCtx) -> bool {
        let Some(snapshot) = ctx.tree_snapshot_arc() else {
            return false;
        };
        let mut ids = Vec::new();
        if !collect(
            &snapshot,
            self.id(),
            "connection",
            &[(
                self.motion_data_mode.is_bound().then(|| self.motion_data_mode.id()),
                "motion_data_mode",
            )],
            &mut ids,
        ) {
            return false;
        }
        self.finish_guards(ctx, ids, &[])
    }
    pub(super) fn guard_result(&self, ctx: &mut ProcessCtx, native: &[RevisionGuard]) -> bool {
        let Some(snapshot) = ctx.tree_snapshot_arc() else {
            return false;
        };
        let mut ids = Vec::new();
        if !collect(
            &snapshot,
            self.id(),
            "values/info",
            &[(self.last_event.is_bound().then(|| self.last_event.id()), "last_event")],
            &mut ids,
        ) || !collect(
            &snapshot,
            self.id(),
            "connection",
            &[(
                self.motion_data_mode.is_bound().then(|| self.motion_data_mode.id()),
                "motion_data_mode",
            )],
            &mut ids,
        ) {
            return false;
        }
        self.finish_guards(ctx, ids, native)
    }
    pub(super) fn guard_activity(&self, ctx: &mut ProcessCtx) -> bool {
        let Some(snapshot) = ctx.tree_snapshot_arc() else {
            return false;
        };
        let mut ids = Vec::new();
        if !collect(
            &snapshot,
            self.id(),
            "values/info",
            &[(self.activity.is_bound().then(|| self.activity.id()), "activity")],
            &mut ids,
        ) {
            return false;
        }
        self.finish_guards(ctx, ids, &[])
    }
    pub(super) fn guard_projection(&self, ctx: &mut ProcessCtx, native: &[RevisionGuard]) -> bool {
        let Some(snapshot) = ctx.tree_snapshot_arc() else {
            return false;
        };
        let mut ids = Vec::with_capacity(64);
        macro_rules! group {
            ($path:literal, $($field:ident),+ $(,)?) => {
                if !collect(&snapshot, self.id(), $path, &[$((self.$field.is_bound().then(|| self.$field.id()), stringify!($field))),+], &mut ids) { return false; }
            };
        }
        group!("connection", motion_data_mode, processing_fps_cap);
        if !collect(
            &snapshot,
            self.id(),
            "connection",
            &[(self.base.connection_status_parameter(), "connected")],
            &mut ids,
        ) {
            return false;
        }
        group!("parameters/stick_processing", stick_dead_zone);
        group!(
            "values/info",
            activity,
            is_connected,
            left_controller_connected,
            right_controller_connected,
            last_event
        );
        group!("values/left_controller", left_stick);
        group!(
            "values/left_controller/left_buttons",
            left_dpad_down,
            left_dpad_up,
            left_dpad_right,
            left_dpad_left,
            left_sl,
            left_sr,
            left_l,
            left_zl,
            left_minus,
            left_stick_button,
            left_capture
        );
        group!(
            "values/left_controller/left_motion",
            left_orientation,
            left_accelerometer,
            left_gyroscope
        );
        group!("values/right_controller", right_stick);
        group!(
            "values/right_controller/right_buttons",
            right_y,
            right_x,
            right_b,
            right_a,
            right_sr,
            right_sl,
            right_r,
            right_zr,
            right_plus,
            right_stick_button,
            right_home
        );
        group!(
            "values/right_controller/right_motion",
            right_orientation,
            right_accelerometer,
            right_gyroscope
        );
        self.finish_guards(ctx, ids, native)
    }
    fn projection_settings(&self) -> runtime::Settings {
        runtime::Settings {
            dead_zone: self.stick_dead_zone.get(),
            motion: self.motion_data_mode(),
        }
    }
    pub(super) fn configure_projection(&mut self, ctx: &mut ProcessCtx) {
        if let Some(runtime) = &self.runtime {
            if let Err(error) = runtime.configure(self.projection_settings()) {
                self.set_runtime_warning(ctx, &error);
            }
        }
    }
    pub(super) fn drain_runtime_events(&mut self, ctx: &mut ProcessCtx) {
        for _ in 0..2 {
            let Some(runtime) = &self.runtime else {
                break;
            };
            match runtime.try_recv() {
                Ok(publication) => {
                    if !publication.guards.iter().all(RevisionGuard::is_current) {
                        self.set_runtime_warning(ctx, "Joy-Con input scope retired; publication was cancelled");
                        continue;
                    }
                    self.runtime_contact_elapsed = Duration::ZERO;
                    match &publication.event {
                        JoyConWorkerEvent::Heartbeat => {}
                        JoyConWorkerEvent::State(state) => {
                            self.service.connected_observation();
                            let reference = self
                                .pending_state
                                .as_ref()
                                .and_then(|value| value.state())
                                .unwrap_or(&self.last_state);
                            let discrete = connection_state_changed(reference, state)
                                || JoyConSide::ALL
                                    .iter()
                                    .any(|side| controller_buttons_changed(reference.side(*side), state.side(*side)));
                            self.pending_state = Some(publication);
                            if discrete {
                                self.apply_pending_state_if_due(ctx, true);
                            }
                            continue;
                        }
                        JoyConWorkerEvent::Error(error) => {
                            if self.guard_result(ctx, &publication.guards) {
                                self.set_runtime_warning(ctx, error);
                                self.set_last_event(ctx, error.clone());
                            }
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
    }
    pub(super) fn apply_pending_state_if_due(&mut self, ctx: &mut ProcessCtx, force: bool) {
        let Some(publication) = self.pending_state.as_ref().cloned() else {
            return;
        };
        if !force {
            let interval = processing_interval_from_fps_cap(self.processing_fps_cap.get());
            if self.processing_elapsed < interval {
                return;
            }
            self.processing_elapsed = self.processing_elapsed.saturating_sub(interval);
        } else {
            self.processing_elapsed = Duration::ZERO;
        }
        self.pending_state = None;
        if !publication.guards.iter().all(RevisionGuard::is_current)
            || publication.settings != Some(self.projection_settings())
        {
            self.set_runtime_warning(ctx, "Joy-Con projection scope retired; publication was cancelled");
            return;
        }
        if !self.guard_projection(ctx, &publication.guards) {
            self.set_runtime_warning(ctx, "Joy-Con publication target is unavailable");
            return;
        }
        let state = publication.state().unwrap();
        let previous = self.last_state.clone();
        self.clear_runtime_warning(ctx);
        self.note_incoming_state(ctx, &previous, state, self.motion_data_mode());
        self.apply_runtime_state(ctx, state.clone());
        for (row, side) in publication.rows.as_ref().unwrap().iter().zip(JoyConSide::ALL) {
            row.emit(ctx, self.id(), previous.side(side));
        }
        ctx.edits.retain_payload(publication);
    }
}

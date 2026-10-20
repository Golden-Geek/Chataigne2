use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use golden_core::{
    edit::{Edit, EditCompletion, EditCompletionStatus, EditQueue},
    node::{Node, NodeId, NodeWarning},
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};
use golden_io::RetainedPayload;

use super::{
    runtime::{self, ChoicePlan, Preparation, RuntimeWakeSender},
    SoundCardModule,
};

const RETRY_DELAY: Duration = Duration::from_millis(250);
const WARNING_ID: &str = "sound-card-device-choices";

pub(super) struct ChoiceSync {
    dirty: bool,
    request: Option<Preparation<ChoicePlan>>,
    pending: Option<(RetainedPayload<ChoicePlan>, EditCompletion)>,
    applied_revision: Option<u64>,
    failed_revision: Option<u64>,
    retry_at: Option<Instant>,
    warning: Option<String>,
    applied_warning: Option<Option<String>>,
    pending_warning: Option<(Option<String>, EditCompletion)>,
    warning_retry_at: Option<Instant>,
}

impl Default for ChoiceSync {
    fn default() -> Self {
        Self {
            dirty: true,
            request: None,
            pending: None,
            applied_revision: None,
            failed_revision: None,
            retry_at: None,
            warning: None,
            applied_warning: None,
            pending_warning: None,
            warning_retry_at: None,
        }
    }
}

impl ChoiceSync {
    pub(super) fn invalidate(&mut self) {
        self.dirty = true;
        self.request = None;
        self.retry_at = None;
        self.failed_revision = None;
    }

    pub(super) fn pause(&mut self) {
        self.request = None;
        self.dirty = true;
    }

    pub(super) fn needs_snapshot(&self, revision: u64) -> bool {
        if self.pending.is_some() || self.retry_at.is_some_and(|deadline| Instant::now() < deadline) {
            return false;
        }
        if let Some(request) = &self.request {
            return request.is_complete();
        }
        self.dirty || (self.applied_revision != Some(revision) && self.failed_revision != Some(revision))
    }

    pub(super) fn has_pending(&self) -> bool {
        self.request.is_some()
            || self.pending.is_some()
            || self.pending_warning.is_some()
            || self.applied_warning.as_ref() != Some(&self.warning)
    }

    fn settle(&mut self, wake: Option<&RuntimeWakeSender>) {
        let Some((plan, receipt)) = &self.pending else { return };
        match receipt.status() {
            EditCompletionStatus::Pending => {
                if let Some(wake) = wake {
                    wake.wake();
                }
            }
            EditCompletionStatus::Applied => {
                self.applied_revision = Some(plan.device_revision);
                self.failed_revision = None;
                self.pending = None;
                self.warning = None;
            }
            EditCompletionStatus::Failed => {
                self.pending = None;
                self.failure(
                    "Sound Card device-choice publication failed; preparing a fresh result".into(),
                    true,
                    wake,
                );
            }
        }
    }

    fn failure(&mut self, message: String, retryable: bool, wake: Option<&RuntimeWakeSender>) {
        self.request = None;
        self.failed_revision = None;
        self.warning = Some(message);
        self.dirty = retryable;
        self.retry_at = retryable.then(|| Instant::now() + RETRY_DELAY);
        if let Some(wake) = wake {
            if retryable {
                wake.wake_after(RETRY_DELAY);
            } else {
                wake.wake();
            }
        }
    }

    fn service(
        &mut self,
        ctx: &mut ProcessCtx,
        module: NodeId,
        wake: Option<&RuntimeWakeSender>,
        enabled: bool,
        snapshot: Option<&Arc<ProcessTreeSnapshot>>,
        inventory: Option<runtime::observation::DeviceInventory>,
    ) {
        self.settle(wake);
        self.service_warning(ctx, module, wake);
        if !enabled {
            self.pause();
            return;
        }
        if self.pending.is_some() {
            return;
        }
        let Some(wake) = wake else { return };
        if let Some(deadline) = self.retry_at {
            if let Some(remaining) = deadline.checked_duration_since(Instant::now()) {
                wake.wake_after(remaining);
                return;
            }
            self.retry_at = None;
        }
        let Some(inventory) = inventory else {
            self.invalidate();
            return;
        };
        let revision = inventory.revision();
        if self
            .request
            .as_ref()
            .is_some_and(|request| !request.matches_device_revision(revision))
        {
            self.invalidate();
        }
        if !self.needs_snapshot(revision) {
            return;
        }
        let Some(snapshot) = snapshot else {
            // A completion or deadline can arrive after this wake was declared
            // snapshot-free. Request one continuation at that boundary.
            wake.wake();
            return;
        };
        if self
            .request
            .as_ref()
            .is_some_and(|request| !request.matches(snapshot, module) || !request.matches_device_revision(revision))
        {
            self.invalidate();
        }
        if self.request.is_none() {
            match Preparation::start_choices(snapshot, module, inventory, wake.clone()) {
                Ok(request) => {
                    self.request = Some(request);
                    self.dirty = false;
                }
                Err(error) => {
                    let retryable = error.retryable;
                    self.failure(error.message, error.retryable, Some(wake));
                    if !retryable {
                        self.failed_revision = Some(revision);
                    }
                }
            }
            return;
        }
        let request = self.request.as_mut().unwrap();
        let ready = request.poll();
        if !request.matches(snapshot, module) || !request.matches_device_revision(revision) {
            self.invalidate();
            wake.wake();
            return;
        }
        match ready {
            Ok(None) => {} // Native completion supplies the next wake.
            Err(error) => {
                let retryable = error.retryable;
                self.failure(error.message, error.retryable, Some(wake));
                if !retryable {
                    self.failed_revision = Some(revision);
                }
            }
            Ok(Some(plan)) => {
                let patches = plan.take_patches().expect("choice plans publish once");
                let mut queue = EditQueue::new();
                for patch in patches {
                    queue.push(Edit::SetParamConstraints {
                        node: patch.node,
                        constraints: patch.constraints,
                    });
                }
                let receipt = queue.completion();
                ctx.edits.pending.extend(queue.drain());
                self.request = None;
                self.pending = Some((plan, receipt));
                wake.wake();
            }
        }
    }

    fn service_warning(&mut self, ctx: &mut ProcessCtx, module: NodeId, wake: Option<&RuntimeWakeSender>) {
        if let Some((desired, receipt)) = &self.pending_warning {
            match receipt.status() {
                EditCompletionStatus::Pending => {
                    if let Some(wake) = wake {
                        wake.wake();
                    }
                    return;
                }
                EditCompletionStatus::Applied => {
                    self.applied_warning = Some(desired.clone());
                    self.pending_warning = None;
                }
                EditCompletionStatus::Failed => {
                    self.pending_warning = None;
                    self.warning_retry_at = Some(Instant::now() + RETRY_DELAY);
                }
            }
        }
        if self.applied_warning.as_ref() == Some(&self.warning) {
            return;
        }
        if let Some(deadline) = self.warning_retry_at {
            if let Some(remaining) = deadline.checked_duration_since(Instant::now()) {
                if let Some(wake) = wake {
                    wake.wake_after(remaining);
                }
                return;
            }
            self.warning_retry_at = None;
        }
        let mut queue = EditQueue::new();
        queue.push(match &self.warning {
            Some(message) => Edit::SetNodeWarning {
                node: module,
                warning: NodeWarning {
                    id: WARNING_ID.into(),
                    message: message.clone(),
                    detail: None,
                },
            },
            None => Edit::ClearNodeWarning {
                node: module,
                warning_id: Some(WARNING_ID.into()),
            },
        });
        let receipt = queue.completion();
        ctx.edits.pending.extend(queue.drain());
        self.pending_warning = Some((self.warning.clone(), receipt));
        if let Some(wake) = wake {
            wake.wake();
        }
    }
}

impl SoundCardModule {
    pub(super) fn service_device_choices(&mut self, ctx: &mut ProcessCtx) {
        let snapshot = ctx.tree_snapshot_arc();
        let inventory = self
            .runtime
            .as_ref()
            .and_then(runtime::SoundCardRuntime::device_inventory);
        let module = self.id();
        self.choices.service(
            ctx,
            module,
            self.runtime_wake.as_ref(),
            self.node_data().effective_enabled,
            snapshot.as_ref(),
            inventory,
        );
    }
}

#[cfg(test)]
mod tests;

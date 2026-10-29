use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use golden_io::WorkerNotifier;

use super::surface::Surface;
use super::{SourceSnapshot, SourceState};

#[derive(Clone)]
pub(super) struct Publication {
    pub surface: Arc<Surface>,
    pub sequence: u64,
}

pub(super) struct Shared {
    pub observation_revision: golden_io::RevisionSource,
    pub source_notifier: Arc<Mutex<Option<WorkerNotifier>>>,
    pub latest: Mutex<Option<Publication>>,
    pub status: Arc<Mutex<SourceSnapshot>>,
    pub active: AtomicBool,
    pub demands: AtomicUsize,
    pub quiescing: AtomicBool,
    pub retiring: AtomicBool,
    pub released: AtomicBool,
    pub unlock_failed: AtomicBool,
    pub shutdown: AtomicBool,
    pub notify: Option<WorkerNotifier>,
    pub worker_notify: Mutex<Option<WorkerNotifier>>,
}

impl Shared {
    pub fn publish(&self, publication: Publication) -> bool {
        let mut latest = self.latest.lock().unwrap_or_else(|error| error.into_inner());
        if !self.active.load(Ordering::Acquire)
            || self.retiring.load(Ordering::Acquire)
            || self.shutdown.load(Ordering::Acquire)
        {
            return false;
        }
        let sequence = publication.sequence;
        *latest = Some(publication);
        self.update(|status| {
            status.sequence = sequence;
            status.publications = status.publications.saturating_add(1);
            status.state = SourceState::Streaming;
        });
        drop(latest);
        self.wake_renderer();
        true
    }

    pub fn clear(&self) {
        self.latest.lock().unwrap_or_else(|error| error.into_inner()).take();
    }

    pub fn wake_renderer(&self) {
        if let Some(notify) = &self.notify {
            notify.notify();
        }
    }

    fn update_demand(&self, add: bool) {
        let mut latest = self.latest.lock().unwrap_or_else(|error| error.into_inner());
        let count = self.demands.load(Ordering::Acquire);
        let count = if add {
            count.checked_add(1)
        } else {
            count.checked_sub(1)
        }
        .expect("bounded source demand count");
        self.demands.store(count, Ordering::Release);
        let active = count != 0 && !self.quiescing.load(Ordering::Acquire) && !self.shutdown.load(Ordering::Acquire);
        let changed = self.active.swap(active, Ordering::AcqRel) != active;
        if !active {
            latest.take();
        }
        drop(latest);
        if changed && let Some(notify) = &*self.worker_notify.lock().unwrap_or_else(|error| error.into_inner()) {
            notify.notify();
        }
    }

    pub fn acquire_demand(self: &Arc<Self>) -> Demand {
        self.update_demand(true);
        Demand(self.clone())
    }

    pub fn quiesce(&self) {
        let mut latest = self.latest.lock().unwrap_or_else(|error| error.into_inner());
        self.quiescing.store(true, Ordering::Release);
        self.active.store(false, Ordering::Release);
        latest.take();
        drop(latest);
        if let Some(notify) = &*self.worker_notify.lock().unwrap_or_else(|error| error.into_inner()) {
            notify.notify();
        }
    }

    pub fn update(&self, update: impl FnOnce(&mut SourceSnapshot)) {
        let mut status = self.status.lock().unwrap_or_else(|error| error.into_inner());
        let before = (
            status.state,
            status.observed_descriptor,
            status.generation,
            status.pool_generation,
        );
        let layout_before = (status.observed_descriptor, status.generation, status.pool_generation);
        update(&mut status);
        if layout_before != (status.observed_descriptor, status.generation, status.pool_generation) {
            self.observation_revision.advance();
        }
        let changed = before
            != (
                status.state,
                status.observed_descriptor,
                status.generation,
                status.pool_generation,
            );
        drop(status);
        if changed {
            self.notify_source();
        }
    }

    fn notify_source(&self) {
        if let Some(notifier) = &*self.source_notifier.lock().unwrap_or_else(|error| error.into_inner()) {
            notifier.notify();
        }
    }

    pub fn state(&self, state: SourceState) {
        let mut status = self.status.lock().unwrap_or_else(|error| error.into_inner());
        let changed = status.state != state;
        if changed && state == SourceState::Stopped {
            self.observation_revision.advance();
        }
        status.state = state;
        drop(status);
        if changed {
            self.wake_renderer();
            self.notify_source();
        }
    }
}

pub(super) struct Demand(Arc<Shared>);

impl Drop for Demand {
    fn drop(&mut self) {
        self.0.update_demand(false);
    }
}

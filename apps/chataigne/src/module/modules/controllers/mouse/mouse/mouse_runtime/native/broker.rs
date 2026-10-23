use super::*;
use golden_io::RetirementPermit;
use std::sync::Weak;

const MAX_SUBSCRIBERS: usize = 4;

// The registry is accessed only by input-owner workers, never by the control engine.
pub(super) struct Registry<D> {
    current: Mutex<Weak<Hub<D>>>,
    lifetime: RetirementPool,
}
impl<D: Send + Sync + 'static> Registry<D> {
    pub(super) fn new() -> Self {
        Self {
            current: Mutex::new(Weak::new()),
            lifetime: RetirementPool::new(NonZeroUsize::MIN),
        }
    }
    #[cfg(test)]
    pub(super) fn subscribe(
        &self,
        config: MouseInputConfig,
        open: impl FnOnce(Broadcast) -> Result<D, String>,
    ) -> Result<Subscription<D>, String> {
        self.subscribe_inner(config, None, open)
    }
    pub(super) fn subscribe_guarded(
        &self,
        config: MouseInputConfig,
        guard: RevisionGuard,
        open: impl FnOnce(Broadcast) -> Result<D, String>,
    ) -> Result<Subscription<D>, String> {
        self.subscribe_inner(config, Some(guard), open)
    }
    fn subscribe_inner(
        &self,
        config: MouseInputConfig,
        guard: Option<RevisionGuard>,
        open: impl FnOnce(Broadcast) -> Result<D, String>,
    ) -> Result<Subscription<D>, String> {
        if config.selection.capacity() > MAX_DEVICE_TEXT {
            return Err("Mouse selection exceeds 1024 bytes".into());
        }
        let mut current = self.current.lock().unwrap_or_else(|error| error.into_inner());
        if let Some(hub) = current.upgrade() {
            return hub.subscribe(config, guard);
        }
        // Weak upgrade fails as soon as the last subscriber starts dropping. The permit
        // survives its endpoint join, preventing a replacement registration during cleanup.
        let lease = self
            .lifetime
            .try_reserve()
            .map_err(|_| "Mouse shared input endpoint is retiring")?;
        let bus = Broadcast::default();
        let (receiver, failure) = bus.subscribe(config, guard)?;
        let endpoint = open(bus.clone())?;
        let hub = Arc::new(Hub {
            bus,
            _endpoint: endpoint,
            _lease: lease,
        });
        *current = Arc::downgrade(&hub);
        Ok(Subscription { receiver, failure, hub })
    }
}
struct Hub<D> {
    bus: Broadcast,
    _endpoint: D,
    _lease: RetirementPermit,
}
impl<D> Hub<D> {
    fn subscribe(
        self: &Arc<Self>,
        config: MouseInputConfig,
        guard: Option<RevisionGuard>,
    ) -> Result<Subscription<D>, String> {
        let (receiver, failure) = self.bus.subscribe(config, guard)?;
        Ok(Subscription {
            receiver,
            failure,
            hub: self.clone(),
        })
    }
}
impl<D> Drop for Hub<D> {
    fn drop(&mut self) {
        self.bus.fail("Mouse shared input endpoint retired".into());
    }
}

#[derive(Default)]
struct State {
    subscribers: Vec<(Sender<MouseRuntimeEvent>, MouseInputConfig, Option<RevisionGuard>)>,
    catalog: Option<Vec<DiscoveredMouseDevice>>,
    error: Option<String>,
    on_change: Option<WorkerNotifier>,
}
#[derive(Clone, Default)]
pub(super) struct Broadcast(Arc<Mutex<State>>);
impl Broadcast {
    fn subscribe(
        &self,
        config: MouseInputConfig,
        guard: Option<RevisionGuard>,
    ) -> Result<(PendingReceiver<MouseRuntimeEvent>, Failure), String> {
        let mut state = self.0.lock().unwrap_or_else(|error| error.into_inner());
        if let Some(error) = &state.error {
            return Err(error.clone());
        }
        if state.subscribers.len() >= MAX_SUBSCRIBERS {
            return Err("Mouse shared input's four subscriber lifetimes are occupied".into());
        }
        let (sender, receiver, failure) = channel();
        // Seed the latest catalog under the same lock as fan-out, before any new edge.
        if let Some(devices) = &state.catalog {
            sender.send(MouseRuntimeEvent::DevicesChanged(devices.clone()))?;
        }
        state.subscribers.push((sender, config, guard));
        let notifier = state.on_change.clone();
        drop(state);
        if let Some(notifier) = notifier {
            notifier.notify();
        }
        Ok((receiver, failure))
    }
    fn unsubscribe(&self, failure: &Failure) {
        let mut state = self.0.lock().unwrap_or_else(|error| error.into_inner());
        state
            .subscribers
            .retain(|(sender, _, _)| !Arc::ptr_eq(&sender.failure, failure));
        let notifier = state.on_change.clone();
        drop(state);
        if let Some(notifier) = notifier {
            notifier.notify();
        }
    }
    pub(super) fn notify_changes(&self, notifier: WorkerNotifier) {
        self.0.lock().unwrap_or_else(|error| error.into_inner()).on_change = Some(notifier);
    }
    pub(super) fn send(&self, event: MouseRuntimeEvent) -> Result<(), String> {
        if let Err(error) = event.weight() {
            self.fail(error.clone());
            return Err(error);
        }
        let mut state = self.0.lock().unwrap_or_else(|error| error.into_inner());
        if let Some(error) = &state.error {
            return Err(error.clone());
        }
        if let MouseRuntimeEvent::DevicesChanged(devices) = &event {
            state.catalog = Some(devices.clone());
        }
        // One subscriber's rejection is terminal for that subscriber, never for its peers.
        let mut accepted = false;
        for (sender, _, guard) in &state.subscribers {
            if guard.as_ref().is_some_and(|guard| !guard.is_current()) {
                sender.fail("Mouse input subscriber retired".into());
                continue;
            }
            if !sender.failure.stopped.load(Ordering::Acquire) && sender.send(event.clone()).is_ok() {
                accepted = true;
            }
        }
        if !accepted {
            let error = "Mouse shared input has no accepting subscribers".to_owned();
            state.error = Some(error.clone());
            return Err(error);
        }
        Ok(())
    }
    pub(super) fn capture_enabled(&self) -> bool {
        self.0
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .subscribers
            .iter()
            .any(|(sender, config, guard)| {
                config.capture_os_input
                    && guard.as_ref().is_none_or(RevisionGuard::is_current)
                    && !sender.failure.stopped.load(Ordering::Acquire)
            })
    }
    pub(super) fn capture_target(&self, devices: &[DiscoveredMouseDevice], device: &str) -> bool {
        self.0
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .subscribers
            .iter()
            .any(|(sender, config, guard)| {
                config.capture_os_input
                    && guard.as_ref().is_none_or(RevisionGuard::is_current)
                    && !sender.failure.stopped.load(Ordering::Acquire)
                    && super::super::super::selected_mouse_device(&config.selection, devices)
                        .is_some_and(|selected| selected.variant_id == device)
            })
    }
    pub(super) fn has_capture_selection(&self, devices: &[DiscoveredMouseDevice]) -> bool {
        devices
            .iter()
            .any(|device| self.capture_target(devices, &device.variant_id))
    }
    pub(super) fn fail(&self, error: String) {
        let mut state = self.0.lock().unwrap_or_else(|error| error.into_inner());
        if state.error.is_some() {
            return;
        }
        let mut end = error.len().min(1024);
        while !error.is_char_boundary(end) {
            end -= 1;
        }
        let error = error[..end].to_owned();
        for (sender, _, _) in state.subscribers.drain(..) {
            sender.fail(error.clone());
        }
        state.error = Some(error);
    }
}
pub(super) struct Stopped(pub(super) Broadcast);
impl Drop for Stopped {
    fn drop(&mut self) {
        self.0.fail("Mouse shared input worker stopped unexpectedly".into());
    }
}

pub(super) struct Subscription<D> {
    receiver: PendingReceiver<MouseRuntimeEvent>,
    failure: Failure,
    hub: Arc<Hub<D>>,
}
impl<D: Send + 'static> Provider for Subscription<D> {
    fn poll(&mut self) -> Result<Vec<MouseRuntimeEvent>, String> {
        drain(&self.receiver, &self.failure)
    }
}
impl<D> Drop for Subscription<D> {
    fn drop(&mut self) {
        self.hub.bus.unsubscribe(&self.failure);
    }
}
impl Record for MouseRuntimeEvent {
    fn weight(&self) -> Result<usize, String> {
        validate(self)
    }
}

#[cfg(test)]
mod tests;

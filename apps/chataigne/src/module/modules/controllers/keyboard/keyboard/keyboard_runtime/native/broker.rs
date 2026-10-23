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
    pub(super) fn subscribe(
        &self,
        open: impl FnOnce(Broadcast) -> Result<D, String>,
    ) -> Result<Subscription<D>, String> {
        let mut current = self.current.lock().unwrap_or_else(|error| error.into_inner());
        if let Some(hub) = current.upgrade() {
            return hub.subscribe();
        }
        // Weak upgrade fails as soon as the last subscriber starts dropping. The permit
        // survives its endpoint join, preventing a replacement registration during cleanup.
        let lease = self
            .lifetime
            .try_reserve()
            .map_err(|_| "Keyboard shared input endpoint is retiring")?;
        let bus = Broadcast::default();
        let (receiver, failure) = bus.subscribe()?;
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
    fn subscribe(self: &Arc<Self>) -> Result<Subscription<D>, String> {
        let (receiver, failure) = self.bus.subscribe()?;
        Ok(Subscription {
            receiver,
            failure,
            hub: self.clone(),
        })
    }
}
impl<D> Drop for Hub<D> {
    fn drop(&mut self) {
        self.bus.fail("Keyboard shared input endpoint retired".into());
    }
}

#[derive(Default)]
struct State {
    subscribers: Vec<Sender<KeyboardRuntimeEvent>>,
    catalog: Option<Vec<DiscoveredKeyboardDevice>>,
    error: Option<String>,
}
#[derive(Clone, Default)]
pub(super) struct Broadcast(Arc<Mutex<State>>);
impl Broadcast {
    fn subscribe(&self) -> Result<(PendingReceiver<KeyboardRuntimeEvent>, Failure), String> {
        let mut state = self.0.lock().unwrap_or_else(|error| error.into_inner());
        if let Some(error) = &state.error {
            return Err(error.clone());
        }
        if state.subscribers.len() >= MAX_SUBSCRIBERS {
            return Err("Keyboard shared input's four subscriber lifetimes are occupied".into());
        }
        let (sender, receiver, failure) = channel();
        // Seed the latest catalog under the same lock as fan-out, before any new edge.
        if let Some(devices) = &state.catalog {
            sender.send(KeyboardRuntimeEvent::DevicesChanged(devices.clone()))?;
        }
        state.subscribers.push(sender);
        Ok((receiver, failure))
    }
    fn unsubscribe(&self, failure: &Failure) {
        self.0
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .subscribers
            .retain(|sender| !Arc::ptr_eq(&sender.failure, failure));
    }
    pub(super) fn send(&self, event: KeyboardRuntimeEvent) -> Result<(), String> {
        if let Err(error) = event.weight() {
            self.fail(error.clone());
            return Err(error);
        }
        let mut state = self.0.lock().unwrap_or_else(|error| error.into_inner());
        if let Some(error) = &state.error {
            return Err(error.clone());
        }
        if let KeyboardRuntimeEvent::DevicesChanged(devices) = &event {
            state.catalog = Some(devices.clone());
        }
        // One subscriber's rejection is terminal for that subscriber, never for its peers.
        let mut accepted = false;
        for sender in &state.subscribers {
            if !sender.failure.stopped.load(Ordering::Acquire) && sender.send(event.clone()).is_ok() {
                accepted = true;
            }
        }
        if !accepted {
            let error = "Keyboard shared input has no accepting subscribers".to_owned();
            state.error = Some(error.clone());
            return Err(error);
        }
        Ok(())
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
        for sender in state.subscribers.drain(..) {
            sender.fail(error.clone());
        }
        state.error = Some(error);
    }
}
pub(super) struct Stopped(pub(super) Broadcast);
impl Drop for Stopped {
    fn drop(&mut self) {
        self.0.fail("Keyboard shared input worker stopped unexpectedly".into());
    }
}

pub(super) struct Subscription<D> {
    receiver: PendingReceiver<KeyboardRuntimeEvent>,
    failure: Failure,
    hub: Arc<Hub<D>>,
}
impl<D: Send + 'static> Provider for Subscription<D> {
    fn poll(&mut self) -> Result<Vec<KeyboardRuntimeEvent>, String> {
        drain(&self.receiver, &mut Vec::new(), &self.failure, |event| event)
    }
}
impl<D> Drop for Subscription<D> {
    fn drop(&mut self) {
        self.hub.bus.unsubscribe(&self.failure);
    }
}
impl Record for KeyboardRuntimeEvent {
    fn weight(&self) -> Result<usize, String> {
        match self {
            Self::DevicesChanged(devices) => catalog_weight(devices),
            Self::Input { device, .. } if device.len() <= MAX_DEVICE_TEXT => Ok(INPUT_WEIGHT),
            _ => Err("Keyboard native input device id exceeds 1024 bytes".into()),
        }
    }
}

#[cfg(test)]
mod tests;

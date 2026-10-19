use super::*;
use std::sync::atomic::AtomicBool;

/// Lifetime of a UI connection. Hosts close it when the physical connection ends.
/// Clones identify the same lifetime; a replacement connection gets a fresh lease.
#[derive(Clone)]
pub struct UiClientLease {
    client_instance_id: Option<String>,
    closed: Arc<AtomicBool>,
}

impl UiClientLease {
    /// Creates an independent connection lifetime, optionally sharing a browser identity.
    pub fn new(client_instance_id: Option<&str>) -> Self {
        Self {
            client_instance_id: client_instance_id.map(str::to_owned),
            closed: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Binds handshake identity while preserving the physical connection's close signal.
    pub fn with_client_instance_id(&self, client_instance_id: &str) -> Self {
        Self {
            client_instance_id: Some(client_instance_id.to_owned()),
            closed: self.closed.clone(),
        }
    }

    /// Fences queued requests without needing admission to the actor queue.
    pub fn close(&self) {
        self.closed.store(true, Ordering::Release);
    }

    /// Returns whether the host has closed this connection.
    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire)
    }

    /// Stable browser identity, when supplied by the client handshake.
    pub fn client_instance_id(&self) -> Option<&str> {
        self.client_instance_id.as_deref()
    }
}

pub(super) struct UiSessionLease {
    lease: UiClientLease,
    edit_id: String,
}

impl UiSessionLease {
    fn matches<T: Node>(&self, engine: &Engine<T>) -> bool {
        engine.active_edit_session_origin() == Some(crate::edit::EditOrigin::Ui)
            && engine.active_edit_session_id() == Some(self.edit_id.as_str())
            && engine.active_edit_session_ui_client_instance_id() == self.lease.client_instance_id()
    }
}

impl<T: Node> ProductionState<T> {
    /// A handshake ends any anonymous edit before named requests use the same connection.
    pub(crate) fn release_rebound_ui_session(&mut self, client: Option<&UiClientLease>) -> bool {
        let Some((owner, client)) = self.ui_session_lease.as_ref().zip(client) else {
            return false;
        };
        if !Arc::ptr_eq(&owner.lease.closed, &client.closed)
            || owner.lease.client_instance_id() == client.client_instance_id()
        {
            return false;
        }
        let owner = self.ui_session_lease.take().expect("rebound session owner");
        self.engine
            .finish_ui_edit_session(owner.lease.client_instance_id(), &owner.edit_id)
    }

    /// The next actor operation observes closure even when the queue was previously full.
    pub(crate) fn release_closed_ui_session(&mut self) -> bool {
        let Some(owner) = &self.ui_session_lease else {
            return false;
        };
        // Live connections cost one atomic read per actor turn; compare identifiers
        // only when releasing a closed lifetime.
        if !owner.lease.is_closed() {
            return false;
        }
        if !owner.matches(&self.engine) {
            self.ui_session_lease = None;
            return false;
        }
        let owner = self.ui_session_lease.take().expect("closed session owner");
        self.engine
            .finish_ui_edit_session(owner.lease.client_instance_id(), &owner.edit_id)
    }

    pub(crate) fn track_ui_session_lease(&mut self, lease: Option<&UiClientLease>, began: bool) {
        if self
            .ui_session_lease
            .as_ref()
            .is_some_and(|owner| !owner.matches(&self.engine))
        {
            self.ui_session_lease = None;
        }
        if began
            && self.ui_session_lease.is_none()
            && let Some(lease) = lease
            && self.engine.active_edit_session_origin() == Some(crate::edit::EditOrigin::Ui)
            && self.engine.active_edit_session_ui_client_instance_id() == lease.client_instance_id()
            && let Some(edit_id) = self.engine.active_edit_session_id()
        {
            self.ui_session_lease = Some(UiSessionLease {
                lease: lease.clone(),
                edit_id: edit_id.to_owned(),
            });
        }
    }
}

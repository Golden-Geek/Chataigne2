//! Deterministic in-memory transports for module and protocol tests.

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

use crate::BoundedQueue;

pub struct TestTransportEndpoint<T> {
    incoming: Arc<Mutex<BoundedQueue<T>>>,
    outgoing: Arc<Mutex<BoundedQueue<T>>>,
    connected: Arc<AtomicBool>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum TestTransportSendError<T> {
    Disconnected(T),
    Full(T),
}

pub fn test_transport_pair<T>(
    maximum_items: usize,
    maximum_weight: usize,
) -> (TestTransportEndpoint<T>, TestTransportEndpoint<T>) {
    let left_incoming = Arc::new(Mutex::new(BoundedQueue::new(maximum_items, maximum_weight)));
    let right_incoming = Arc::new(Mutex::new(BoundedQueue::new(maximum_items, maximum_weight)));
    let connected = Arc::new(AtomicBool::new(true));

    (
        TestTransportEndpoint {
            incoming: Arc::clone(&left_incoming),
            outgoing: Arc::clone(&right_incoming),
            connected: Arc::clone(&connected),
        },
        TestTransportEndpoint {
            incoming: right_incoming,
            outgoing: left_incoming,
            connected,
        },
    )
}

impl<T> TestTransportEndpoint<T> {
    pub fn send(&self, value: T, weight: usize) -> Result<(), TestTransportSendError<T>> {
        if !self.is_connected() {
            return Err(TestTransportSendError::Disconnected(value));
        }
        self.outgoing
            .lock()
            .expect("test transport queue is not poisoned")
            .try_push(value, weight)
            .map_err(|error| TestTransportSendError::Full(error.into_inner()))
    }

    pub fn try_receive(&self) -> Option<T> {
        self.incoming
            .lock()
            .expect("test transport queue is not poisoned")
            .pop_front()
    }

    pub fn disconnect(&self) {
        self.connected.store(false, Ordering::Release);
    }

    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Acquire)
    }
}

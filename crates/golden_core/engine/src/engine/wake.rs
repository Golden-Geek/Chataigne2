use std::{
    collections::HashMap,
    fmt,
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender, TrySendError},
    },
};

use crate::edit::{EditCompletion, EditCompletionStatus};
use crate::node::NodeId;

const MAXIMUM_NODE_WAKERS: usize = 1024;

/// Failure to reserve a worker wake source before starting its producer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeWakeError {
    /// The owning engine or node has gone away.
    Closed,
    /// A standalone processing context has no engine wake source.
    Unavailable,
    /// The engine's live/retiring wake registrations occupy every reserved slot.
    Capacity,
}

impl fmt::Display for NodeWakeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Closed => "node wake destination is closed",
            Self::Unavailable => "node wake registration requires an engine ready callback",
            Self::Capacity => "engine node wake registration capacity is exhausted",
        })
    }
}

impl std::error::Error for NodeWakeError {}

enum WakeState {
    Idle,
    Queued { after: Option<EditCompletion>, again: bool },
    Running { again: bool },
    Closed,
}

struct WakeSlot {
    node: NodeId,
    state: Mutex<WakeState>,
    source: NodeWakeSource,
}

/// Cloneable, coalesced worker notification for one node in one engine lifetime.
///
/// Reserve it before accepting worker work. Signals retain no payload: a module must own its
/// bounded result mailbox and consume it in `Node::on_worker_wake`. Multiple producers for a node share
/// one reservation. Waking during a callback schedules another turn; it cannot be cleared by
/// that callback's acknowledgement. This is a level notification, not an ordered command queue.
#[derive(Clone)]
pub struct NodeWaker(Arc<WakeSlot>);

impl PartialEq for NodeWaker {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for NodeWaker {}

impl NodeWaker {
    /// Requests an update without waiting for queue capacity, even for a passive node.
    pub fn wake(&self) -> Result<(), NodeWakeError> {
        let mut state = self.0.state.lock().unwrap_or_else(|p| p.into_inner());
        if !self.0.source.0.alive.load(Ordering::Acquire) {
            *state = WakeState::Closed;
        }
        match &mut *state {
            WakeState::Closed => Err(NodeWakeError::Closed),
            WakeState::Queued { after, again } => {
                if after.is_some() {
                    *again = true;
                }
                Ok(())
            }
            WakeState::Running { again } => {
                *again = true;
                Ok(())
            }
            WakeState::Idle => {
                *state = WakeState::Queued {
                    after: None,
                    again: false,
                };
                self.0.enqueue(&mut state)
            }
        }
    }
}

impl fmt::Debug for NodeWaker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NodeWaker")
            .field("node", &self.0.node)
            .finish_non_exhaustive()
    }
}

impl WakeSlot {
    fn enqueue(self: &Arc<Self>, state: &mut WakeState) -> Result<(), NodeWakeError> {
        match self.source.0.sender.try_send(Arc::clone(self)) {
            Ok(()) => Ok(()),
            Err(TrySendError::Disconnected(_)) => {
                *state = WakeState::Closed;
                Err(NodeWakeError::Closed)
            }
            // Every queued/running slot retains its registration. Only Idle->Queued or the
            // unique running delivery can enqueue it, so registered cardinality bounds depth.
            Err(TrySendError::Full(_)) => unreachable!("node wake reservation invariant violated"),
        }
    }
}

struct WakeRegistry {
    sender: SyncSender<Arc<WakeSlot>>,
    registrations: Mutex<Registrations>,
    alive: AtomicBool,
}

#[derive(Default)]
struct Registrations {
    by_node: HashMap<NodeId, Weak<WakeSlot>>,
    // Closed generations remain reserved while a worker or delivery still owns them.
    slots: Vec<Weak<WakeSlot>>,
}

#[derive(Clone)]
pub(crate) struct NodeWakeSource(Arc<WakeRegistry>);

impl NodeWakeSource {
    pub(super) fn close(&self, node: NodeId) {
        let registrations = self.0.registrations.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(slot) = registrations.by_node.get(&node).and_then(Weak::upgrade) {
            *slot.state.lock().unwrap_or_else(|p| p.into_inner()) = WakeState::Closed;
        }
    }

    pub(crate) fn register(&self, node: NodeId) -> Result<NodeWaker, NodeWakeError> {
        if !self.0.alive.load(Ordering::Acquire) {
            return Err(NodeWakeError::Closed);
        }
        let mut registrations = self.0.registrations.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(slot) = registrations.by_node.get(&node).and_then(Weak::upgrade) {
            let closed = matches!(*slot.state.lock().unwrap_or_else(|p| p.into_inner()), WakeState::Closed);
            if !closed {
                return Ok(NodeWaker(slot));
            }
        }
        registrations.by_node.retain(|_, slot| slot.strong_count() != 0);
        registrations.slots.retain(|slot| slot.strong_count() != 0);
        if registrations.slots.len() == MAXIMUM_NODE_WAKERS {
            return Err(NodeWakeError::Capacity);
        }
        let slot = Arc::new(WakeSlot {
            node,
            state: Mutex::new(WakeState::Idle),
            source: self.clone(),
        });
        registrations.by_node.insert(node, Arc::downgrade(&slot));
        registrations.slots.push(Arc::downgrade(&slot));
        Ok(NodeWaker(slot))
    }
}

pub(super) struct NodeWakeQueue {
    pub(super) source: NodeWakeSource,
    receiver: Receiver<Arc<WakeSlot>>,
}

impl NodeWakeQueue {
    pub(super) fn new() -> Self {
        let (sender, receiver) = mpsc::sync_channel(MAXIMUM_NODE_WAKERS);
        Self {
            source: NodeWakeSource(Arc::new(WakeRegistry {
                sender,
                registrations: Mutex::new(Registrations::default()),
                alive: AtomicBool::new(true),
            })),
            receiver,
        }
    }

    pub(super) fn take_turn(&self, maximum: usize) -> Vec<WakeDelivery> {
        let mut turn = Vec::new();
        // Extract the complete bounded turn before callbacks can re-arm their own slot.
        for slot in self.receiver.try_iter().take(maximum) {
            let after = {
                let mut state = slot.state.lock().unwrap_or_else(|p| p.into_inner());
                if matches!(*state, WakeState::Closed) {
                    continue;
                }
                let WakeState::Queued { after, again } =
                    std::mem::replace(&mut *state, WakeState::Running { again: false })
                else {
                    unreachable!("only queued slots enter wake channel")
                };
                *state = WakeState::Running { again };
                after
            };
            turn.push(WakeDelivery {
                slot,
                complete: false,
                after,
            });
        }
        turn
    }
}

impl<T: crate::node::Node> super::Engine<T> {
    /// Reserves a worker wake destination for an attached node before admitting worker work.
    /// Registrations are limited to 1024 live/retiring sources per engine and reused per node.
    pub fn node_waker(&self, node: NodeId) -> Result<NodeWaker, NodeWakeError> {
        if !self.nodes.contains(node) {
            return Err(NodeWakeError::Closed);
        }
        self.node_wakes.source.register(node)
    }
}

impl Drop for NodeWakeQueue {
    fn drop(&mut self) {
        self.source.0.alive.store(false, Ordering::Release);
    }
}

/// Dropped/unprocessed deliveries requeue themselves after an earlier callback fails.
pub(super) struct WakeDelivery {
    slot: Arc<WakeSlot>,
    complete: bool,
    after: Option<EditCompletion>,
}

impl WakeDelivery {
    pub(super) fn ready(&mut self) -> bool {
        match self.after.as_ref().map(EditCompletion::status) {
            Some(EditCompletionStatus::Pending) => return false,
            Some(EditCompletionStatus::Applied) => {
                let state = self.slot.state.lock().unwrap_or_else(|p| p.into_inner());
                if matches!(*state, WakeState::Running { again: false }) {
                    self.complete = true;
                    return false;
                }
            }
            Some(EditCompletionStatus::Failed) | None => {}
        }
        self.after = None;
        *self.slot.state.lock().unwrap_or_else(|p| p.into_inner()) = WakeState::Running { again: false };
        true
    }

    pub(super) fn wait_for(mut self, completion: EditCompletion) {
        self.after = Some(completion);
    }

    pub(super) fn node(&self) -> NodeId {
        self.slot.node
    }
    pub(super) fn complete(mut self) {
        self.complete = true;
    }
    pub(super) fn close(self) {
        *self.slot.state.lock().unwrap_or_else(|p| p.into_inner()) = WakeState::Closed;
    }
}

impl Drop for WakeDelivery {
    fn drop(&mut self) {
        let mut state = self.slot.state.lock().unwrap_or_else(|p| p.into_inner());
        if matches!(*state, WakeState::Closed) {
            return;
        }
        if self.complete && matches!(*state, WakeState::Running { again: false }) {
            *state = WakeState::Idle;
        } else {
            let again = matches!(*state, WakeState::Running { again: true });
            *state = WakeState::Queued {
                after: self.after.take(),
                again,
            };
            let _ = self.slot.enqueue(&mut state);
        }
    }
}

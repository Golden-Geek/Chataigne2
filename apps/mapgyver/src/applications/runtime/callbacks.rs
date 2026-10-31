//! Ordered, bounded process-phase delivery; UI observation coalescing is a separate stream.
use super::*;
use golden_core::{
    edit::{Edit, EditCompletion, EditCompletionStatus},
    events::{CustomEvent, CustomEventRetention},
    script::ScriptCustomEvent,
};
use std::{collections::VecDeque, time::Duration};

const CAPACITY: usize = 1024;
// Starting, readiness, ready, stopping, unverified retirement and terminal: at most six
// published phases per launch; keep two spare credits for cancellation/failure edges.
const LAUNCH_EVENTS: usize = 8;
const BATCH_EVENTS: usize = 16;
pub const TOPIC: &str = "mapgyver.application.lifecycle";

#[derive(Default)]
pub(super) struct State {
    queue: VecDeque<ApplicationLifecycleEvent>,
    reserved: usize,
    sequence: u64,
    acknowledged: u64,
    fault: Option<String>,
    batch: Option<Arc<Batch>>,
}
impl State {
    pub(super) fn snapshot(&self) -> ApplicationCallbackState {
        ApplicationCallbackState {
            pending: self.queue.len() as u32,
            acknowledged: self.acknowledged,
            fault: self.fault.clone(),
        }
    }
}
#[derive(Default)]
pub(super) struct Journal {
    pub(super) state: Mutex<State>,
    wake: Mutex<Option<(uuid::Uuid, WorkerNotifier)>>,
}
pub(super) struct Reservation {
    journal: Arc<Journal>,
    remaining: usize,
}
impl Drop for Reservation {
    fn drop(&mut self) {
        self.journal.state.lock().unwrap().reserved -= self.remaining;
    }
}
struct Batch {
    token: uuid::Uuid,
    through: u64,
    payloads: Vec<golden_io::SharedPayload<serde_json::Value>>,
    receipt: OnceLock<EditCompletion>,
}
impl Journal {
    pub(super) fn notify(&self) {
        if let Some((_, notifier)) = &*self.wake.lock().unwrap() {
            notifier.notify();
        }
    }
    pub(super) fn reserve(self: &Arc<Self>) -> Option<Reservation> {
        let mut state = self.state.lock().unwrap();
        if state.fault.is_some() || state.queue.len() + state.reserved + LAUNCH_EVENTS > CAPACITY {
            return None;
        }
        state.reserved += LAUNCH_EVENTS;
        Some(Reservation {
            journal: self.clone(),
            remaining: LAUNCH_EVENTS,
        })
    }
    pub(super) fn append(
        &self,
        reservation: Option<&mut Reservation>,
        project: uuid::Uuid,
        elapsed_ms: u64,
        status: ApplicationRuntimeStatus,
    ) {
        let mut state = self.state.lock().unwrap();
        let Some(reservation) = reservation.filter(|permit| permit.remaining > 0) else {
            state.fault =
                Some("lifecycle transition exceeded its reserved callback budget; reload the project to resume".into());
            return;
        };
        reservation.remaining -= 1;
        state.reserved -= 1;
        state.sequence = state
            .sequence
            .checked_add(1)
            .expect("application lifecycle sequence exhausted");
        let sequence = state.sequence;
        state.queue.push_back(ApplicationLifecycleEvent {
            project,
            sequence,
            elapsed_ms,
            status,
        });
    }
}

#[derive(Clone)]
pub struct ApplicationCallbacks {
    project: uuid::Uuid,
    guard: RevisionGuard,
    pub(super) view: RuntimeView,
}
pub struct ApplicationCallbackSubscription {
    _worker: BoundedWorker<(), ()>,
}
struct ConsumerLease {
    owner: uuid::Uuid,
    journal: Arc<Journal>,
}
impl Drop for ConsumerLease {
    fn drop(&mut self) {
        let mut wake = self.journal.wake.lock().unwrap();
        if wake.as_ref().is_some_and(|(owner, _)| *owner == self.owner) {
            wake.take();
        }
    }
}
impl ApplicationCallbacks {
    pub(super) fn new(project: uuid::Uuid, guard: RevisionGuard, view: RuntimeView) -> Self {
        Self { project, guard, view }
    }
    pub fn project(&self) -> uuid::Uuid {
        self.project
    }
    pub fn is_current(&self) -> bool {
        self.guard.is_current()
    }
    /// The sole host consumer prepares callback payloads off the actor. Failed edit receipts
    /// halt further launch admission: an applied prefix must never be blindly replayed.
    /// The sink returns after its public project transaction settles; pre-dispatch errors retry.
    pub fn start(
        &self,
        deliver: impl Fn(ApplicationCallbackTicket) -> Result<(), String> + Send + 'static,
    ) -> Result<ApplicationCallbackSubscription, String> {
        if !self.is_current() {
            return Err("callback project is retired".into());
        }
        let owner = uuid::Uuid::new_v4();
        let mut wake = self.view.callbacks.wake.lock().unwrap();
        if wake.is_some() {
            return Err("application callback consumer is already owned".into());
        }
        *wake = Some((owner, WorkerNotifier::new(|| {})));
        drop(wake);
        static POOL: OnceLock<RetirementPool> = OnceLock::new();
        let pool = POOL.get_or_init(|| RetirementPool::new(NonZeroUsize::new(2).unwrap()));
        let source = self.clone();
        // Keep ownership until an in-flight sink has actually returned, including after cancellation.
        let lease = ConsumerLease {
            owner,
            journal: self.view.callbacks.clone(),
        };
        let worker = match BoundedWorker::spawn(
            "mapgyver-app-callbacks",
            pool,
            WorkerLimits {
                command_items: 1,
                command_weight: 1,
                event_items: 1,
                event_weight: 1,
            },
            move |context| {
                let _lease = lease;
                while !context.cancellation().is_cancelled() && source.is_current() {
                    let checkpoint = context.wake_checkpoint();
                    let result = source.prepare().and_then(|ticket| match ticket {
                        Some(ticket) => {
                            let delivered = deliver(ticket);
                            source.settle();
                            delivered.map(|_| true)
                        }
                        None => Ok(false),
                    });
                    let delay = match result {
                        Ok(true) => Duration::from_millis(1),
                        Ok(false) => Duration::from_secs(60),
                        Err(_) => Duration::from_secs(1),
                    };
                    context.wait_until(checkpoint, Instant::now() + delay);
                }
            },
        ) {
            Ok(worker) => worker,
            Err(error) => {
                return Err(error.to_string());
            }
        };
        let notifier = worker.wake_notifier();
        let mut wake = self.view.callbacks.wake.lock().unwrap();
        if !wake.as_ref().is_some_and(|(current, _)| *current == owner) {
            return Err("application callback consumer retired during startup".into());
        }
        *wake = Some((owner, notifier.clone()));
        drop(wake);
        notifier.notify();
        Ok(ApplicationCallbackSubscription { _worker: worker })
    }
    pub(super) fn prepare(&self) -> Result<Option<ApplicationCallbackTicket>, String> {
        self.settle();
        if !self.is_current() {
            return Err("callback project is retired".into());
        }
        let events = {
            let state = self.view.callbacks.state.lock().unwrap();
            if let Some(fault) = &state.fault {
                return Err(fault.clone());
            }
            if let Some(batch) = &state.batch {
                return Ok(Some(ApplicationCallbackTicket {
                    project: self.project,
                    batch: batch.token,
                }));
            }
            state.queue.iter().take(BATCH_EVENTS).cloned().collect::<Vec<_>>()
        };
        if events.is_empty() {
            return Ok(None);
        }
        let through = events.last().unwrap().sequence;
        let mut payloads = Vec::with_capacity(events.len());
        for event in events {
            let value = serde_json::json!({ "callback": "onApplicationLifecycle", "args": [event] });
            let prepared = ScriptCustomEvent::prepare(TOPIC, value).map_err(|error| error.to_string())?;
            payloads.push(prepared.into_shared_parts().1);
        }
        let batch = Arc::new(Batch {
            token: uuid::Uuid::new_v4(),
            through,
            payloads,
            receipt: OnceLock::new(),
        });
        let ticket = ApplicationCallbackTicket {
            project: self.project,
            batch: batch.token,
        };
        self.view.callbacks.state.lock().unwrap().batch = Some(batch);
        Ok(Some(ticket))
    }
    pub(super) fn settle(&self) {
        let mut state = self.view.callbacks.state.lock().unwrap();
        let Some(batch) = state.batch.clone() else { return };
        match batch.receipt.get().map(EditCompletion::status) {
            Some(EditCompletionStatus::Applied) => {
                while state.queue.front().is_some_and(|event| event.sequence <= batch.through) {
                    state.queue.pop_front();
                }
                state.acknowledged = batch.through;
                state.batch = None;
                self.view.changed();
            }
            Some(EditCompletionStatus::Failed) if state.fault.is_none() => {
                state.fault = Some("lifecycle callback transaction failed; partial delivery cannot be replayed; reload the project to resume".into());
                self.view.changed();
            }
            _ => {}
        }
    }
}

pub(crate) fn dispatch(
    engine: &mut AppEngine,
    ticket: ApplicationCallbackTicket,
) -> Result<crate::media::protocol::MediaEditResult, String> {
    let manager = crate::media::authoring::manager(engine)?;
    let Some(AppNode::MediaManager(node)) = engine.nodes.get(manager) else {
        unreachable!()
    };
    if node.instance != ticket.project {
        return Err("lifecycle callback belongs to a stale project".into());
    }
    let batch = node
        .applications
        .callbacks
        .state
        .lock()
        .unwrap()
        .batch
        .clone()
        .filter(|batch| batch.token == ticket.batch)
        .ok_or("callback batch is not admitted")?;
    if batch.receipt.get().is_some() {
        return Err("callback batch is already queued".into());
    }
    for payload in &batch.payloads {
        engine.edits.push(Edit::EmitCustomEvent {
            event: CustomEvent::from_shared_payload(
                TOPIC,
                Some(manager),
                payload.clone(),
                CustomEventRetention::Transient,
            ),
        });
    }
    batch
        .receipt
        .set(engine.edits.completion())
        .map_err(|_| "callback batch is already queued")?;
    Ok(crate::media::protocol::MediaEditResult::Changed)
}

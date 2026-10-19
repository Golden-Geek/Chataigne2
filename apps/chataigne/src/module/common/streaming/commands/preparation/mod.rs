#[cfg(test)]
mod tests;
mod worker;

use super::{bounds::json_weight, CommandCapture, StreamingSendRequest};
use golden_core::{
    edit::{EditCompletion, EditCompletionStatus},
    engine::NodeWaker,
    events::CustomEvent,
    node::NodeId,
    parameter::ParamValue,
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};
use golden_io::{PayloadPool, RetainedPayload, RetirementPermit, WorkerNotifier};
use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, OnceLock,
    },
};

pub(crate) const PREPARED_COMMAND_TOPIC: &str = "chataigne.stream.prepared_command";
const MAX_OWNER_COMMANDS: usize = 16;
const OVERRIDE_BYTES: usize = 256 * 1024;

pub(crate) struct PreparedCommand {
    pub module: NodeId,
    pub request: StreamingSendRequest,
    source: Arc<AtomicBool>,
    target: NodeWaker,
}
impl PreparedCommand {
    pub fn is_live(&self) -> bool {
        !self.source.load(Ordering::Acquire) && !self.target.is_closed()
    }
}
pub(crate) fn prepared_command(event: &CustomEvent, module: NodeId) -> Option<&RetainedPayload<PreparedCommand>> {
    if event.topic != PREPARED_COMMAND_TOPIC {
        return None;
    }
    event
        .local_payload::<RetainedPayload<PreparedCommand>>()
        .filter(|command| command.module == module)
}

struct State {
    result: OnceLock<Result<RetainedPayload<PreparedCommand>, String>>,
    cancelled: Arc<AtomicBool>,
    notifier: WorkerNotifier,
    _admission: RetirementPermit,
}
#[derive(Clone)]
struct Ticket(Arc<State>);
impl Ticket {
    fn cancelled(&self) -> bool {
        self.0.cancelled.load(Ordering::Acquire)
    }
    fn complete(&self, result: Result<RetainedPayload<PreparedCommand>, String>) {
        let result = result.map_err(|error| {
            if error.capacity() > 4096 {
                "Stream preparation failed; error details exceed 4096 bytes".into()
            } else {
                error
            }
        });
        if self.0.result.set(result).is_ok() {
            self.0.notifier.notify();
        }
    }
}
struct Delivery {
    ticket: Ticket,
    completion: Option<EditCompletion>,
}

#[derive(Default)]
pub(crate) struct CommandPreparation {
    pending: VecDeque<Delivery>,
    wake: Option<(NodeWaker, WorkerNotifier)>,
    cancelled: Option<Arc<AtomicBool>>,
    #[cfg(test)]
    service: Option<Arc<worker::Service>>,
}
impl CommandPreparation {
    pub fn enqueue(
        &mut self,
        ctx: &ProcessCtx,
        snapshot: &Arc<ProcessTreeSnapshot>,
        command: NodeId,
        capture: CommandCapture,
        execution: Option<Arc<serde_json::Value>>,
    ) -> Result<(), String> {
        let result = self.enqueue_inner(ctx, snapshot, command, capture, execution);
        if result.is_err() && self.pending.is_empty() {
            self.wake = None;
        }
        result
    }
    fn enqueue_inner(
        &mut self,
        ctx: &ProcessCtx,
        snapshot: &Arc<ProcessTreeSnapshot>,
        command: NodeId,
        capture: CommandCapture,
        execution: Option<Arc<serde_json::Value>>,
    ) -> Result<(), String> {
        if snapshot.node(command).is_none_or(|node| !node.enabled) {
            return Err("Stream command source is absent or disabled; command was not admitted".into());
        }
        if self.pending.len() >= MAX_OWNER_COMMANDS {
            return Err("Stream command owner already has 16 admitted preparations".into());
        }
        #[cfg(test)]
        let service = self.service.clone().unwrap_or_else(|| worker::SERVICE.clone());
        #[cfg(not(test))]
        let service = worker::SERVICE.clone();
        let admission = service
            .admission
            .try_reserve()
            .map_err(|_| "Stream preparation admission is full; command was not admitted")?;
        let override_weight = execution
            .as_ref()
            .map(|value| json_weight(value, OVERRIDE_BYTES, 4096, 16))
            .transpose()?;
        let target_parameter =
            snapshot.find_child_by_decl_id(command, crate::app::module_command::MODULE_COMMAND_TARGET_MODULE_PATH);
        // Only dispatch-target validation happens here. All other override decoding,
        // overlay creation, parameter collection and encoding belong to the native job.
        let target_override = target_parameter
            .and_then(|target| {
                execution
                    .as_ref()?
                    .get("param_overrides")?
                    .as_array()?
                    .iter()
                    .rev()
                    .find(|value| value.get("param_id").and_then(serde_json::Value::as_u64) == Some(target.0))
                    .and_then(|value| value.get("value"))
            })
            .map(|value| {
                json_weight(value, 4096, 128, 8)?;
                serde_json::from_value::<ParamValue>(value.clone())
                    .map_err(|_| "Invalid stream target override".to_string())
            })
            .transpose()?;
        let module = crate::app::module_command::module_command_target(snapshot, command, target_override.as_ref())?;
        if !snapshot.node(module).is_some_and(|node| {
            matches!(
                node.node_type.as_str(),
                "udp_module"
                    | "tcp_client_module"
                    | "tcp_server_module"
                    | "websocket_client_module"
                    | "websocket_server_module"
                    | "serial_module"
            )
        }) {
            return Err("Stream command target is not a supported stream module".into());
        }
        let target = ctx.node_waker_for(module).map_err(|error| error.to_string())?;
        let source = ctx.node_waker().map_err(|error| error.to_string())?;
        if self.wake.as_ref().is_some_and(|(old, _)| *old != source) {
            self.clear();
        }
        let cancelled = self
            .cancelled
            .get_or_insert_with(|| Arc::new(AtomicBool::new(false)))
            .clone();
        let notifier = self
            .wake
            .get_or_insert_with(|| {
                let waker = source.clone();
                (
                    source,
                    WorkerNotifier::new(move || {
                        let _ = waker.wake();
                    }),
                )
            })
            .1
            .clone();
        let snapshot = snapshot.try_lease().map_err(|error| error.to_string())?;
        let execution = execution
            .zip(override_weight)
            .map(|(value, weight)| retain_execution(value, weight))
            .transpose()?;
        let ticket = Ticket(Arc::new(State {
            result: OnceLock::new(),
            cancelled,
            notifier,
            _admission: admission,
        }));
        service.submit(worker::Job {
            snapshot,
            execution,
            command,
            capture,
            ticket: ticket.clone(),
            target,
            module,
        })?;
        self.pending.push_back(Delivery {
            ticket,
            completion: None,
        });
        Ok(())
    }
    pub fn service(&mut self, ctx: &mut ProcessCtx) {
        let Some(front) = self.pending.front_mut() else {
            self.wake = None;
            return;
        };
        if let Some(receipt) = front.completion.take() {
            match receipt.status() {
                EditCompletionStatus::Pending => {
                    front.completion = Some(receipt);
                    self.request_service();
                    return;
                }
                EditCompletionStatus::Applied => {
                    self.pending.pop_front();
                    self.rearm();
                    return;
                }
                EditCompletionStatus::Failed => {}
            }
        }
        let Some(result) = front.ticket.0.result.get() else {
            return;
        };
        match result {
            Err(error) => {
                golden_core::logerror!(format!("Stream command preparation failed: {}", error));
                self.pending.pop_front();
            }
            Ok(command) => {
                let previous = std::mem::take(&mut ctx.edits);
                ctx.emit_custom_event(CustomEvent::local(
                    PREPARED_COMMAND_TOPIC,
                    Some(command.module),
                    command.clone(),
                ));
                let mut edits = std::mem::replace(&mut ctx.edits, previous);
                front.completion = Some(edits.completion());
                ctx.edits.pending.extend(edits.drain());
                self.request_service();
            }
        }
        self.rearm();
    }
    fn request_service(&self) {
        if let Some((_, notifier)) = &self.wake {
            notifier.notify();
        }
    }
    fn rearm(&mut self) {
        if self.pending.is_empty() {
            self.wake = None;
        } else if self
            .pending
            .front()
            .is_some_and(|front| front.completion.is_some() || front.ticket.0.result.get().is_some())
        {
            self.request_service();
        }
    }
    pub fn clear(&mut self) {
        if let Some(cancelled) = self.cancelled.take() {
            cancelled.store(true, Ordering::Release);
        }
        self.pending.clear();
        self.wake = None;
    }
}
impl Drop for CommandPreparation {
    fn drop(&mut self) {
        self.clear();
    }
}

fn retain_execution(
    value: Arc<serde_json::Value>,
    weight: usize,
) -> Result<RetainedPayload<serde_json::Value>, String> {
    static POOL: Mutex<Option<PayloadPool>> = Mutex::new(None);
    let mut pool = POOL.lock().unwrap_or_else(|p| p.into_inner());
    if pool.is_none() {
        *pool = Some(
            PayloadPool::new("stream-command-overrides", 64, 64 * OVERRIDE_BYTES).map_err(|error| error.to_string())?,
        );
    }
    pool.as_ref()
        .unwrap()
        .try_retain_arc(value, weight)
        .map_err(|_| "Stream override lifetime capacity exhausted; command was not admitted".into())
}

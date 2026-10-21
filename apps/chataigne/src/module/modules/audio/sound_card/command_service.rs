//! App-owned command preparation. The worker never mutates the graph or submits native
//! commands; the actor admits the prepared request once, then publishes its receipts.
use std::{
    num::NonZeroUsize,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, OnceLock,
    },
};

use golden_audio::{AudioError, CommandSequence};
use golden_core::{
    edit::Edit,
    engine::NodeWaker,
    events::CustomEvent,
    node::{Node, NodeId, NodeUuid, NodeWarning},
    parameter::{ParamValue, ParameterEventBehaviour},
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
    script::ScriptArguments,
};
use golden_io::{
    BoundedWorker, PayloadPool, PayloadReservation, PendingDrainState, RetainedPayload, RetirementPool, WorkerLimits,
    WorkerNotifier,
};

use super::{
    integration::{SoundCardCommandResultEvent, SOUND_CARD_COMMAND_RESULT_TOPIC},
    runtime::RuntimeWakeSender,
    SoundCardModule,
};
use crate::app::module::{common::edit_receipt::EditReceipt, script_api::CallbackDelivery};
use crate::app::{
    module_command::CapturedInvocation,
    module_modules_audio_sound_card_commands::{prepare_command, SoundCardCommandRequest},
};

const REQUEST_BYTES: usize = 4 * 1024 * 1024;
const REQUESTS: usize = 64;
const TURN: usize = 4;

enum Source {
    Command(CapturedInvocation),
    Script {
        method: &'static str,
        args: ScriptArguments,
        snapshot: RetainedPayload<ProcessTreeSnapshot>,
    },
}

impl Source {
    fn snapshot(&self) -> &ProcessTreeSnapshot {
        match self {
            Self::Command(source) => &source.snapshot,
            Self::Script { snapshot, .. } => snapshot,
        }
    }
    fn prepare(&self) -> Result<SoundCardCommandRequest, String> {
        match self {
            Self::Command(source) => prepare_command(source),
            Self::Script { method, args, .. } => {
                super::script::script_request(method, args).expect("registered Sound Card script method")
            }
        }
    }
    fn origin(&self) -> Option<(NodeId, &'static str)> {
        let Self::Command(source) = self else {
            return None;
        };
        let kind = source.snapshot.node(source.command)?.node_type.as_str();
        super::SOUND_CARD_COMMAND_TYPES
            .iter()
            .find(|candidate| **candidate == kind)
            .map(|kind| (source.command, *kind))
    }
}

struct Job {
    source: Source,
    producer: NodeWaker,
    lifetime: Arc<AtomicBool>,
    module: NodeId,
    reservation: PayloadReservation,
}

struct GainChange {
    node: NodeId,
    uuid: NodeUuid,
    parent: Option<NodeId>,
    value: ParamValue,
}

struct Prepared {
    // Canonical retention keeps captured input and rejected/unused requests off the actor's
    // final-disposal path, including cancellation and a full result mailbox.
    source: Source,
    producer: NodeWaker,
    lifetime: Arc<AtomicBool>,
    request: Mutex<Option<Result<SoundCardCommandRequest, String>>>,
    gain: Option<GainChange>,
}

impl Prepared {
    fn live(&self) -> bool {
        !self.producer.is_closed() && self.lifetime.load(Ordering::Acquire)
    }
    fn gain_matches(&self, snapshot: &ProcessTreeSnapshot) -> bool {
        self.gain.as_ref().is_none_or(|gain| {
            snapshot.node(gain.node).is_some_and(|node| {
                node.uuid == gain.uuid
                    && node.parent == gain.parent
                    && matches!(node.param_value, Some(ParamValue::Float(_)))
            })
        })
    }
}

#[derive(Default)]
pub(super) struct CommandService {
    worker: Option<BoundedWorker<Job, RetainedPayload<Prepared>>>,
    pending: Option<Delivery>,
}

struct Delivery {
    prepared: RetainedPayload<Prepared>,
    outcome: Option<Result<CommandSequence, AudioError>>,
    admitted_sequence: Option<CommandSequence>,
    gain: EditReceipt,
    gain_done: bool,
    warning: EditReceipt,
    callback: Option<CallbackDelivery>,
    report_ready: bool,
}

impl CommandService {
    fn enqueue(&mut self, job: Job, wake: &RuntimeWakeSender) -> Result<(), String> {
        if self
            .worker
            .as_ref()
            .is_some_and(|worker| !worker.is_accepting_commands())
        {
            return Err("Sound Card command worker stopped; request was not admitted".into());
        }
        if self.worker.is_none() {
            static LIFETIMES: OnceLock<RetirementPool> = OnceLock::new();
            let pool = LIFETIMES.get_or_init(|| RetirementPool::new(NonZeroUsize::new(8).unwrap()));
            let wake = wake.clone();
            self.worker = Some(
                BoundedWorker::spawn_notified(
                    "sound-card-commands",
                    pool,
                    WorkerLimits {
                        command_items: REQUESTS,
                        command_weight: REQUESTS * REQUEST_BYTES,
                        event_items: REQUESTS,
                        event_weight: REQUESTS * REQUEST_BYTES,
                    },
                    WorkerNotifier::new(move || wake.wake()),
                    |ctx: golden_io::WorkerContext<Job, RetainedPayload<Prepared>>| {
                        while let Some(job) = ctx.receive() {
                            if job.producer.is_closed() {
                                continue;
                            }
                            let mut result = if job.lifetime.load(Ordering::Acquire) {
                                job.source.prepare().map_err(bounded_error)
                            } else {
                                Err("Sound Card runtime retired before command preparation".into())
                            };
                            let gain = match &result {
                                Ok(request) => match super::integration::command_gain_change(
                                    job.source.snapshot(),
                                    job.module,
                                    request,
                                ) {
                                    Ok(Some((node, value))) => {
                                        job.source.snapshot().node(node).map(|state| GainChange {
                                            node,
                                            uuid: state.uuid,
                                            parent: state.parent,
                                            value,
                                        })
                                    }
                                    Ok(None) => None,
                                    Err(error) => {
                                        result = Err(bounded_error(error.to_string()));
                                        None
                                    }
                                },
                                Err(_) => None,
                            };
                            let prepared = Prepared {
                                source: job.source,
                                producer: job.producer,
                                lifetime: job.lifetime,
                                request: Mutex::new(Some(result)),
                                gain,
                            };
                            if prepared.producer.is_closed() || ctx.cancellation().is_cancelled() {
                                continue;
                            }
                            let Ok(prepared) = job.reservation.retain(prepared, REQUEST_BYTES) else {
                                unreachable!("fixed admitted weight");
                            };
                            if ctx.publish_weighted(prepared, REQUEST_BYTES).is_err() {
                                break;
                            }
                        }
                    },
                )
                .map_err(|error| format!("Sound Card command preparation unavailable: {error}"))?,
            );
        }
        self.worker
            .as_ref()
            .unwrap()
            .send_weighted(job, REQUEST_BYTES)
            .map_err(|error| match error {
                golden_io::PendingSendError::Full(_) => {
                    "Sound Card command mailbox is full; request was not admitted".into()
                }
                golden_io::PendingSendError::Disconnected(_) => {
                    "Sound Card command worker stopped; request was not admitted".into()
                }
            })
    }

    pub(super) fn needs_snapshot(&self) -> bool {
        self.pending.as_ref().is_some_and(|delivery| {
            !delivery.gain_done
                && delivery.prepared.gain.is_some()
                && delivery.outcome.as_ref().is_none_or(Result::is_ok)
        })
    }

    fn service(&mut self, ctx: &mut ProcessCtx, module: &SoundCardModule) {
        for _ in 0..TURN {
            if self.pending.is_none() {
                let Some(worker) = &self.worker else {
                    break;
                };
                let mut events = Vec::with_capacity(1);
                let drained = worker.drain_events(&mut events, NonZeroUsize::new(1).unwrap());
                self.pending = events.pop().map(|prepared| Delivery {
                    prepared,
                    outcome: None,
                    admitted_sequence: None,
                    gain: EditReceipt::default(),
                    gain_done: false,
                    warning: EditReceipt::default(),
                    callback: None,
                    report_ready: false,
                });
                if matches!(drained.state, PendingDrainState::Disconnected) && self.pending.is_none() {
                    self.worker = None;
                    ctx.set_node_warning_with(
                        module.id(),
                        Some("sound-card-command-worker"),
                        "Sound Card command preparation stopped",
                        Some("Pending native admissions were cancelled; a later command may restart preparation."),
                    );
                }
            }
            let Some(delivery) = self.pending.as_mut() else {
                break;
            };
            if !delivery.service(ctx, module) {
                break;
            }
            self.pending = None;
        }
        if self.pending.is_some() || self.worker.as_ref().is_some_and(BoundedWorker::has_pending) {
            if let Some(wake) = &module.runtime_wake {
                wake.wake();
            }
        }
    }
}

impl Delivery {
    fn service(&mut self, ctx: &mut ProcessCtx, module: &SoundCardModule) -> bool {
        if self.outcome.is_none() {
            let same_runtime = module
                .runtime
                .as_ref()
                .is_some_and(|runtime| Arc::ptr_eq(&runtime.command_lifetime(), &self.prepared.lifetime));
            if !self.prepared.live() || !same_runtime || !module.node_data().effective_enabled {
                self.outcome = Some(Err(super::integration::command_error(
                    "Sound Card command source or runtime retired",
                )));
            } else {
                let snapshot = ctx.tree_snapshot_arc();
                if self.prepared.gain.is_some() && snapshot.is_none() {
                    return false;
                }
                if snapshot
                    .as_ref()
                    .is_some_and(|snapshot| !self.prepared.gain_matches(snapshot))
                {
                    self.outcome = Some(Err(super::integration::command_error(
                        "Sound Card gain target changed before admission",
                    )));
                } else {
                    let request = self
                        .prepared
                        .request
                        .lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .take()
                        .expect("one native admission per prepared command");
                    self.outcome = Some(
                        request
                            .map_err(super::integration::command_error)
                            .and_then(|request| module.admit_prepared_request(snapshot.as_deref(), request)),
                    );
                    self.admitted_sequence = self.outcome.as_ref().unwrap().as_ref().ok().copied();
                }
            }
        }
        if !self.gain_done && self.outcome.as_ref().unwrap().is_ok() {
            if let Some(gain) = &self.prepared.gain {
                let Some(snapshot) = ctx.tree_snapshot_arc() else {
                    return false;
                };
                if !self.prepared.gain_matches(&snapshot) {
                    self.outcome = Some(Err(super::integration::command_error(
                        "Sound Card gain target retired after native admission; command was not replayed",
                    )));
                } else if !self.gain.service(ctx, || Edit::SetParam {
                    node: gain.node,
                    value: gain.value.clone(),
                    behaviour: ParameterEventBehaviour::Coalesce,
                }) {
                    return false;
                } else {
                    self.gain_done = true;
                }
            }
        }
        let outcome = self.outcome.as_ref().unwrap();
        if !self.warning.service(ctx, || match outcome {
            Ok(_) => Edit::ClearNodeWarning {
                node: module.id(),
                warning_id: Some("sound-card-command".into()),
            },
            Err(error) => Edit::SetNodeWarning {
                node: module.id(),
                warning: NodeWarning {
                    id: "sound-card-command".into(),
                    message: "Sound Card command was not completed".into(),
                    detail: Some(error.to_string()),
                },
            },
        }) {
            return false;
        }
        if !self.report_ready {
            if let Some((command_id, kind)) = self.prepared.source.origin() {
                let response = SoundCardCommandResultEvent {
                    module_id: module.id(),
                    command_id,
                    command_type: kind.to_owned(),
                    admitted_sequence: self.admitted_sequence.map(|sequence| sequence.get()),
                    error: outcome.as_ref().err().cloned(),
                };
                let event = CustomEvent::from_payload(SOUND_CARD_COMMAND_RESULT_TOPIC, Some(module.id()), &response)
                    .expect("typed bounded Sound Card result serializes");
                self.callback = Some(CallbackDelivery::new(event));
            }
            if outcome.is_ok() {
                ctx.clear_node_warning(module.id(), Some("sound-card-command-worker"));
                module.base.emit_outgoing_traffic(ctx);
            }
            self.report_ready = true;
        }
        self.callback.as_mut().is_none_or(|callback| callback.service(ctx))
    }
}

fn bounded_error(mut error: String) -> String {
    if error.len() > 4096 {
        let mut end = 4096;
        while !error.is_char_boundary(end) {
            end -= 1;
        }
        error.truncate(end);
        error.shrink_to_fit();
    }
    error
}

fn reservation() -> Result<PayloadReservation, String> {
    static POOL: OnceLock<Mutex<Option<PayloadPool>>> = OnceLock::new();
    let mut pool = POOL
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    if pool.is_none() {
        *pool = Some(
            PayloadPool::new("sound-card-command-values", REQUESTS, REQUESTS * REQUEST_BYTES)
                .map_err(|error| format!("Sound Card command retention unavailable: {error}"))?,
        );
    }
    pool.as_ref()
        .unwrap()
        .try_reserve(REQUEST_BYTES)
        .map_err(|_| "Sound Card command capacity exhausted; request was not admitted".into())
}

impl SoundCardModule {
    fn enqueue_command(&mut self, source: Source, producer: NodeWaker) -> Result<(), String> {
        if !self.node_data().effective_enabled {
            return Err("Sound Card module is disabled".into());
        }
        let runtime = self.runtime.as_ref().ok_or("Sound Card runtime is not available")?;
        let wake = self
            .runtime_wake
            .as_ref()
            .ok_or("Sound Card command notifier is not available")?;
        self.commands.enqueue(
            Job {
                source,
                producer,
                lifetime: runtime.command_lifetime(),
                module: self.id(),
                reservation: reservation()?,
            },
            wake,
        )?;
        Ok(())
    }

    pub(crate) fn dispatch_command(ctx: &mut ProcessCtx, invocation: CapturedInvocation) -> Result<(), String> {
        let module = invocation.module;
        let target = ctx.node_waker_for(module).map_err(|error| error.to_string())?;
        let command_id = invocation.command;
        let kind = invocation
            .snapshot
            .node(command_id)
            .and_then(|node| {
                super::SOUND_CARD_COMMAND_TYPES
                    .iter()
                    .copied()
                    .find(|kind| *kind == node.node_type)
            })
            .ok_or("Unsupported Sound Card command")?;
        let producer = ctx.node_waker().map_err(|error| error.to_string())?;
        ctx.call_node_mutation_without_snapshot(module, move |node, ctx| {
            if target.is_closed() {
                return Ok(());
            }
            let module = node
                .as_any_mut()
                .downcast_mut::<SoundCardModule>()
                .ok_or("Sound Card command target unavailable")?;
            if let Err(error) = module.enqueue_command(Source::Command(invocation), producer) {
                ctx.set_node_warning_with(
                    module.id(),
                    Some("sound-card-command"),
                    "Sound Card command was not admitted",
                    Some(&error),
                );
                let response = SoundCardCommandResultEvent {
                    module_id: module.id(),
                    command_id,
                    command_type: kind.to_owned(),
                    admitted_sequence: None,
                    error: Some(super::integration::command_error(error)),
                };
                ctx.emit_custom_payload(SOUND_CARD_COMMAND_RESULT_TOPIC, Some(module.id()), &response)
                    .map_err(|_| "Failed to emit Sound Card command rejection")?;
            }
            Ok(())
        });
        Ok(())
    }

    pub(super) fn enqueue_script_command(
        &mut self,
        ctx: &mut ProcessCtx,
        method: &'static str,
        args: &[ParamValue],
    ) -> Result<(), String> {
        if args.len() > 4 {
            return Err("Sound Card script command exceeds four arguments".into());
        }
        let snapshot = ctx
            .tree_snapshot_arc()
            .ok_or("Sound Card script command requires a tree snapshot")?;
        let args = ctx.capture_script_args(args)?;
        if args.retained_bytes() > 256 * 1024 {
            return Err("Sound Card script arguments exceed 256 KiB".into());
        }
        self.enqueue_command(
            Source::Script {
                method,
                args,
                snapshot: snapshot.try_lease().map_err(|error| error.to_string())?,
            },
            ctx.node_waker().map_err(|error| error.to_string())?,
        )
    }

    pub(super) fn service_commands(&mut self, ctx: &mut ProcessCtx) {
        let mut commands = std::mem::take(&mut self.commands);
        commands.service(ctx, self);
        self.commands = commands;
    }
}

#[cfg(test)]
mod tests;

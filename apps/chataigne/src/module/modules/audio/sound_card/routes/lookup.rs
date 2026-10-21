//! Ordered routing intents: variable route matching belongs to a bounded worker.
use super::*;
use golden_core::{node::NodeUuid, process_ctx::SubtreeRevision};
use golden_io::{BoundedWorker, RetirementPermit, RetirementPool, WorkerLimits, WorkerNotifier};
use std::{collections::VecDeque, num::NonZeroUsize, sync::OnceLock};

const ROWS: usize = 65_536;
const MAIL_BYTES: usize = 8192;

#[derive(Clone)]
struct Intent {
    direction: golden_audio::AudioDirection,
    key: String,
    channel: NodeUuid,
    connect: bool,
}
struct Accepted {
    intent: Intent,
    selection: Vec<(NodeId, SubtreeRevision)>,
    lifetime: golden_io::RevisionGuard,
    _permit: RetirementPermit,
    retries: usize,
}
struct Job {
    intent: Intent,
    module: NodeId,
    source: golden_io::RetainedPayload<ProcessTreeSnapshot>,
}
struct Prepared {
    job: Job,
    parent: NodeId,
    revision: SubtreeRevision,
    found: Option<NodeId>,
}

#[derive(Default)]
pub(super) struct Lookup {
    queue: VecDeque<Accepted>,
    worker: Option<BoundedWorker<Job, Result<Prepared, String>>>,
    waiting: bool,
    prepared: Option<Result<Prepared, String>>,
    receipt: Option<EditCompletion>,
}

impl Lookup {
    pub(super) fn pending(&self) -> bool {
        pending(&self.receipt)
    }
    pub(in super::super) fn needs_snapshot(&self) -> bool {
        !self.queue.is_empty()
            && !self.pending()
            && (!self.waiting || self.worker.as_ref().is_some_and(BoundedWorker::has_pending))
    }

    fn enqueue(
        &mut self,
        intent: Intent,
        selection: Vec<(NodeId, SubtreeRevision)>,
        lifetime: golden_io::RevisionGuard,
    ) -> Result<(), String> {
        if self.queue.len() >= 32 {
            return Err("Sound Card routing queue is full".into());
        }
        static ADMISSION: OnceLock<RetirementPool> = OnceLock::new();
        let permit = ADMISSION
            .get_or_init(|| RetirementPool::new(NonZeroUsize::new(256).unwrap()))
            .try_reserve()
            .map_err(|_| "Sound Card global routing capacity is exhausted")?;
        self.queue.push_back(Accepted {
            intent,
            selection,
            lifetime,
            _permit: permit,
            retries: 0,
        });
        Ok(())
    }

    fn start(
        &mut self,
        snapshot: &std::sync::Arc<ProcessTreeSnapshot>,
        module: &SoundCardModule,
    ) -> Result<(), String> {
        if self.worker.is_none() {
            static WORKERS: OnceLock<RetirementPool> = OnceLock::new();
            let wake = module
                .runtime_wake
                .clone()
                .ok_or("Sound Card routing notifier is unavailable")?;
            self.worker = Some(
                BoundedWorker::spawn_notified(
                    "sound-card-routing",
                    WORKERS.get_or_init(|| RetirementPool::new(NonZeroUsize::new(8).unwrap())),
                    WorkerLimits {
                        command_items: 1,
                        command_weight: MAIL_BYTES,
                        event_items: 1,
                        event_weight: MAIL_BYTES,
                    },
                    WorkerNotifier::new(move || wake.wake()),
                    |ctx: golden_io::WorkerContext<Job, Result<Prepared, String>>| {
                        while let Some(job) = ctx.receive() {
                            let result = prepare(job, ctx.cancellation());
                            if ctx.publish_weighted(result, MAIL_BYTES).is_err() {
                                break;
                            }
                        }
                    },
                )
                .map_err(|_| "Sound Card routing worker capacity/startup failed")?,
            );
        }
        let job = Job {
            intent: self.queue.front().unwrap().intent.clone(),
            module: module.id(),
            source: snapshot
                .try_lease()
                .map_err(|_| "Sound Card routing snapshot capacity is exhausted")?,
        };
        self.worker
            .as_ref()
            .unwrap()
            .send_weighted(job, MAIL_BYTES)
            .map_err(|error| match error {
                golden_io::PendingSendError::Full(_) => "Sound Card routing worker mailbox is full",
                golden_io::PendingSendError::Disconnected(_) => "Sound Card routing worker stopped",
            })?;
        self.waiting = true;
        Ok(())
    }

    fn service(&mut self, ctx: &mut ProcessCtx, module: &SoundCardModule) {
        for _ in 0..4 {
            if let Some(receipt) = &self.receipt {
                match receipt.status() {
                    EditCompletionStatus::Pending => return,
                    EditCompletionStatus::Applied => {
                        self.queue.pop_front();
                        self.receipt = None;
                    }
                    EditCompletionStatus::Failed => {
                        self.receipt = None;
                        self.queue.front_mut().unwrap().retries += 1;
                    }
                }
            }
            let Some(front) = self.queue.front() else {
                self.worker = None;
                return;
            };
            if let Some(snapshot) = ctx.tree_snapshot() {
                if front
                    .selection
                    .iter()
                    .any(|(id, revision)| snapshot.subtree_revision(*id) != Some(*revision))
                {
                    self.reject(ctx, module, "Sound Card routing intent cancelled by device selection");
                    continue;
                }
            }
            if !front.lifetime.is_current() || front.retries >= 8 {
                self.reject(
                    ctx,
                    module,
                    "Sound Card routing intent cancelled or repeatedly invalidated",
                );
                continue;
            }
            if self.waiting {
                let mut events = Vec::with_capacity(1);
                let Some(worker) = &self.worker else {
                    return;
                };
                let drained = worker.drain_events(&mut events, NonZeroUsize::new(1).unwrap());
                if let Some(reply) = events.pop() {
                    self.prepared = Some(reply);
                    self.waiting = false;
                } else if matches!(drained.state, golden_io::PendingDrainState::Disconnected) {
                    self.reject(ctx, module, "Sound Card routing worker stopped before replying");
                    continue;
                } else {
                    return;
                }
            }
            let Some(snapshot) = ctx.tree_snapshot_arc() else {
                break;
            };
            let intent = &self.queue.front().unwrap().intent;
            if module.require_route_authoring_ready(intent.direction).is_err() {
                self.reject(
                    ctx,
                    module,
                    "Sound Card routing intent cancelled by route reconfiguration",
                );
                continue;
            }
            if let Some(reply) = self.prepared.take() {
                let prepared = match reply {
                    Ok(value) => value,
                    Err(error) => {
                        self.reject(ctx, module, &error);
                        continue;
                    }
                };
                if snapshot.subtree_revision(prepared.parent) != Some(prepared.revision) {
                    self.queue.front_mut().unwrap().retries += 1;
                } else {
                    match module.apply_matched_route(
                        ctx,
                        &snapshot,
                        &prepared.job.intent,
                        prepared.found,
                        self.queue.front().unwrap().lifetime.clone(),
                    ) {
                        Ok(Some(receipt)) => {
                            self.receipt = Some(receipt);
                            break;
                        }
                        Ok(None) => {
                            ctx.clear_node_warning(module.id(), Some("sound-card-routing"));
                            self.queue.pop_front();
                        }
                        Err(error) => self.reject(ctx, module, &error.to_string()),
                    }
                }
            }
            if !self.queue.is_empty() && !self.waiting && self.receipt.is_none() {
                if let Err(error) = self.start(&snapshot, module) {
                    self.reject(ctx, module, &error);
                } else {
                    return;
                }
            }
        }
        if !self.queue.is_empty() {
            if let Some(wake) = &module.runtime_wake {
                wake.wake();
            }
        }
    }

    fn reject(&mut self, ctx: &mut ProcessCtx, module: &SoundCardModule, error: &str) {
        self.queue.pop_front();
        self.prepared = None;
        self.waiting = false;
        self.worker = None;
        ctx.set_node_warning_with(
            module.id(),
            Some("sound-card-routing"),
            "Sound Card routing intent failed",
            Some(error),
        );
        golden_core::logerror!(origin = module.id(); error);
    }
}

fn prepare(job: Job, cancellation: &golden_io::WorkerCancellation) -> Result<Prepared, String> {
    let path = if job.intent.direction == golden_audio::AudioDirection::Input {
        INPUT_ROUTES_PATH
    } else {
        OUTPUT_ROUTES_PATH
    };
    let parent = find_path(&job.source, job.module, path).ok_or("Sound Card routing folder is missing")?;
    let revision = job
        .source
        .subtree_revision(parent)
        .ok_or("Sound Card routing folder retired")?;
    let rows = job.source.child_ids_slice(parent);
    if rows.len() > ROWS {
        return Err("Sound Card routing lookup exceeds 65536 roots".into());
    }
    let mut found = None;
    for (index, &id) in rows.iter().enumerate() {
        if index % 64 == 0 && cancellation.is_cancelled() {
            return Err("Sound Card routing lookup cancelled".into());
        }
        let key = find_path(&job.source, id, "physical_channel")
            .and_then(|id| job.source.node(id))
            .and_then(|node| node.param_value.as_ref());
        let same_key = matches!(key, Some(ParamValue::Str(key) | ParamValue::Enum(key) | ParamValue::File(key)) if key == &job.intent.key);
        let same_channel = find_path(&job.source, id, "channel")
            .and_then(|id| job.source.node(id))
            .and_then(|node| node.param_value.as_ref())
            .is_some_and(|v| matches!(v, ParamValue::Reference(r) if r.uuid() == job.intent.channel));
        if same_key && same_channel {
            found = Some(id);
            break;
        }
    }
    Ok(Prepared {
        job,
        parent,
        revision,
        found,
    })
}

impl SoundCardModule {
    pub(in super::super) fn enqueue_route_intent(
        &mut self,
        snapshot: &ProcessTreeSnapshot,
        direction: golden_audio::AudioDirection,
        key: String,
        uuid: String,
        connect: bool,
    ) -> Result<(), golden_audio::AudioError> {
        self.require_route_authoring_ready(direction)?;
        if self.runtime_wake.is_none() {
            return Err(super::super::integration::command_error(
                "Sound Card routing notifier is unavailable",
            ));
        }
        if key.capacity() > 1024 || uuid.capacity() > 128 {
            return Err(super::super::integration::command_error(
                "Sound Card routing key/UUID exceeds its byte limit",
            ));
        }
        let channel = NodeUuid(
            uuid::Uuid::parse_str(&uuid)
                .map_err(|_| super::super::integration::command_error("Sound Card channel UUID is invalid"))?,
        );
        let lifetime = self.routes.guard();
        let selection = [
            "connection/audio_driver",
            "connection/device",
            "connection/input_device",
            "connection/output_device",
        ]
        .iter()
        .filter_map(|path| {
            let id = find_path(snapshot, self.id(), path)?;
            Some((id, snapshot.subtree_revision(id)?))
        })
        .collect();
        self.routes
            .lookup
            .enqueue(
                Intent {
                    direction,
                    key,
                    channel,
                    connect,
                },
                selection,
                lifetime,
            )
            .map_err(super::super::integration::command_error)?;
        self.wake_route_work();
        Ok(())
    }

    pub(in super::super) fn service_route_intents(&mut self, ctx: &mut ProcessCtx) {
        let mut work = std::mem::take(&mut self.routes.lookup);
        work.service(ctx, self);
        self.routes.lookup = work;
    }

    fn apply_matched_route(
        &self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        intent: &Intent,
        found: Option<NodeId>,
        lifetime: golden_io::RevisionGuard,
    ) -> Result<Option<EditCompletion>, golden_audio::AudioError> {
        let channel = self.resolve_routing_channel_node(snapshot, intent.direction, &intent.channel.0.to_string())?;
        if intent.connect {
            self.validate_physical_channel(intent.direction, &intent.key)?;
        }
        let path = if intent.direction == golden_audio::AudioDirection::Input {
            INPUT_ROUTES_PATH
        } else {
            OUTPUT_ROUTES_PATH
        };
        let parent = find_path(snapshot, self.id(), path)
            .ok_or_else(|| super::super::integration::command_error("Sound Card route folder is missing"))?;
        if (intent.connect && found.is_some()) || (!intent.connect && found.is_none()) {
            return Ok(None);
        }
        let mut guards = Vec::with_capacity(8);
        guards.push(identity(snapshot, self.id()).map_err(super::super::integration::command_error)?);
        guards.push(NodeReadGuard::capture(snapshot, parent).unwrap());
        guards.push(NodeReadGuard::capture(snapshot, channel).unwrap());
        if let Some(route) = found {
            guards.push(NodeReadGuard::capture(snapshot, route).unwrap());
            for path in ["physical_channel", "channel"] {
                if let Some(guard) =
                    find_path(snapshot, route, path).and_then(|id| NodeReadGuard::capture(snapshot, id))
                {
                    guards.push(guard);
                }
            }
        }
        let mut edits = EditQueue::new();
        edits.push(Edit::CheckPreconditions {
            preconditions: EditPreconditions::new(guards, vec![lifetime]).unwrap(),
        });
        if intent.connect {
            let (_, reference) =
                self.resolve_routing_channel(snapshot, intent.direction, &intent.channel.0.to_string())?;
            let mut tree = if intent.direction == golden_audio::AudioDirection::Input {
                NodeTree::new(SoundCardInputRoute::connected(intent.key.clone(), reference))
            } else {
                NodeTree::new(SoundCardOutputRoute::connected(reference, intent.key.clone()))
            };
            structure::set_route_identity(
                tree.node.as_mut(),
                self.node_data().meta.uuid,
                if intent.direction == golden_audio::AudioDirection::Input {
                    "input"
                } else {
                    "output"
                },
                &intent.key,
                intent.channel,
            );
            edits.push(Edit::AddNodeTree {
                parent,
                tree,
                prev_sibling: None,
            });
        } else {
            edits.push(Edit::RemoveNode { node: found.unwrap() });
        }
        let receipt = edits.completion();
        ctx.edits.pending.extend(edits.drain());
        ctx.clear_node_warning(self.id(), Some("sound-card-routing"));
        Ok(Some(receipt))
    }
}

#[cfg(test)]
mod tests;

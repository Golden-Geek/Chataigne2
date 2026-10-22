use super::*;
use golden_core::events::EventKind;

impl DmxModuleBase {
    pub(super) fn capture_output(
        &mut self,
        _ctx: &ProcessCtx,
        input: Input,
        snapshot: RetainedPayload<ProcessTreeSnapshot>,
        producer: NodeWaker,
        time: EngineTime,
    ) -> Result<(), String> {
        if !self.node_data().effective_enabled || self.outgoing.pending.len() + self.pending_sends.len() >= LIMIT {
            return Err("DMX module is disabled or its 64 pending outputs are occupied".into());
        }
        let transport = self
            .transport
            .as_ref()
            .filter(|transport| transport.is_ready())
            .ok_or("DMX endpoint is opening or stopped; request was not admitted")?;
        let session = transport.source_guard().ok_or("DMX session retired")?;
        let key = Key::capture(&snapshot, self.id()).ok_or("DMX source configuration is absent")?;
        let parameters = snapshot
            .find_child_by_decl_id(self.id(), "parameters")
            .ok_or("Missing DMX parameters")?;
        let output_id = snapshot
            .find_child_by_decl_id(parameters, "output_frame")
            .ok_or("Missing DMX output")?;
        let output = (
            output_id,
            snapshot
                .subtree_revision(output_id)
                .ok_or("Missing DMX output revision")?,
        );
        let int = |name| {
            snapshot.find_child_by_decl_id(parameters, name).and_then(|id| {
                match snapshot.node(id)?.param_value.as_ref()? {
                    ParamValue::Int(value) => Some(*value),
                    _ => None,
                }
            })
        };
        let universe = int("universe")
            .and_then(|value| u16::try_from(value).ok())
            .ok_or("Invalid DMX universe")?;
        let priority = int("priority")
            .and_then(|value| u8::try_from(value).ok())
            .ok_or("Invalid DMX priority")?;
        if universe == 0 || universe > self.protocol.maximum_universe() || priority == 0 || priority > 200 {
            return Err("DMX universe or priority is outside the protocol's range".into());
        }
        let scratch = match &input {
            Input::Command(invocation) => 3 * invocation.override_weight,
            _ => 0,
        };
        let weight = WEIGHT + scratch;
        let reservation = pool(false)?
            .try_reserve(weight)
            .map_err(|_| "DMX invocation item/byte capacity exhausted")?;
        let source = reservation
            .retain(
                Source {
                    input,
                    snapshot,
                    producer,
                    session,
                    module: self.id(),
                    key,
                    time,
                    output,
                    universe,
                    priority,
                },
                weight,
            )
            .map_err(|_| "DMX invocation reservation failed")?;
        let index = self
            .outgoing
            .pending
            .partition_point(|pending| pending.source.time <= time);
        self.outgoing.pending.insert(
            index,
            Pending {
                source,
                preparation: None,
                ready: None,
            },
        );
        self.request_service();
        Ok(())
    }

    pub(in super::super) fn capture_script_output(
        &mut self,
        ctx: &mut ProcessCtx,
        method: &str,
        args: &[ParamValue],
    ) -> Option<Result<(), String>> {
        let method = DMX_SCRIPT_METHODS
            .iter()
            .copied()
            .find(|candidate| *candidate == method)?;
        Some((|| {
            let args = ctx.capture_script_args(args).map_err(str::to_owned)?;
            if args.len() > 2 || args.retained_bytes() > 64 * 1024 {
                return Err("DMX scripts accept at most two arguments / 64 KiB retained input".into());
            }
            let snapshot = ctx
                .tree_snapshot_arc()
                .ok_or("Missing DMX script snapshot")?
                .try_lease()
                .map_err(|error| error.to_string())?;
            let producer = ctx.node_waker().map_err(|error| error.to_string())?;
            self.capture_output(ctx, Input::Script { method, args }, snapshot, producer, ctx.time)
        })())
    }

    pub(in super::super) fn capture_property_output(&mut self, ctx: &mut ProcessCtx) -> Result<(), String> {
        let Some(event) = ctx.current_event() else {
            return Ok(());
        };
        let EventKind::ParamChanged {
            new_value: ParamValue::Str(text),
            ..
        } = &event.kind
        else {
            return Ok(());
        };
        if text.len() > frame::MAX_SLOTS_TEXT {
            return Err("DMX channel JSON exceeds 8192 bytes".into());
        }
        let time = event.time;
        // The event owns this occurrence. A later edit may already be visible in
        // the snapshot; copying that value would lose an accepted frame change.
        let text = text.clone();
        let snapshot = ctx
            .tree_snapshot_arc()
            .ok_or("Missing DMX property snapshot")?
            .try_lease()
            .map_err(|error| error.to_string())?;
        let producer = ctx.node_waker().map_err(|error| error.to_string())?;
        self.outgoing.projection = None;
        self.capture_output(ctx, Input::Property(text), snapshot, producer, time)
    }

    pub(in super::super) fn service_output(&mut self, ctx: &mut ProcessCtx) {
        if self.outgoing.retry_at.is_some_and(|at| Instant::now() < at) {
            return;
        }
        self.outgoing.retry_at = None;
        let Some(snapshot) = ctx.tree_snapshot_arc() else {
            return;
        };
        for _ in 0..8 {
            let Some(mut pending) = self.outgoing.pending.pop_front() else {
                break;
            };
            if let Some(reason) = pending.source.rejection(&snapshot) {
                self.set_transport_warning(ctx, reason);
                golden_core::logerror!(reason);
                continue;
            }
            if pending.ready.is_none() {
                if pending.preparation.is_none() {
                    let Some(wake) = &self.wake else {
                        return;
                    };
                    match Preparation::start(pending.source.clone(), wake.notifier.clone()) {
                        Ok(preparation) => {
                            pending.preparation = Some(preparation);
                            self.outgoing.pending.push_front(pending);
                            // Allow inbox stabilization to capture earlier command
                            // and property occurrences before applying any result.
                            return;
                        }
                        Err(_) => {
                            self.outgoing.pending.push_front(pending);
                            self.outgoing.retry_at = Some(Instant::now() + Duration::from_millis(250));
                            wake.later(Duration::from_millis(250));
                            return;
                        }
                    }
                }
                match pending.preparation.as_ref().unwrap().take() {
                    Some(Ok(ready)) => pending.ready = Some(ready),
                    Some(Err(error)) => {
                        self.set_transport_warning(ctx, &error);
                        golden_core::logerror!(error);
                        continue;
                    }
                    None => {
                        self.outgoing.pending.push_front(pending);
                        return;
                    }
                }
            }
            let ready = pending.ready.as_ref().unwrap();
            let source = &ready.source;
            let result = (|| {
                let slots = match &ready.operation {
                    DmxCommandRequest::SetChannel { channel, value } => {
                        let baseline = if self.outgoing.authored == Some(source.output) {
                            &self.output_slots
                        } else {
                            ready.baseline.as_ref().unwrap()
                        };
                        let mut frame = DmxFrame::with_metadata(source.universe, 0, source.priority, baseline.clone())?;
                        frame.set_channel(*channel, *value)?;
                        frame.slots
                    }
                    DmxCommandRequest::SendFrame { slots } => slots.clone(),
                    DmxCommandRequest::Blackout => DmxFrame::blackout(source.universe)?.slots,
                };
                let frame = DmxFrame::with_metadata(source.universe, 0, source.priority, slots)?;
                let transport = self.transport.as_ref().ok_or("DMX transport retired")?;
                let receipt = transport.send_prepared(frame.clone(), ready.clone())?;
                self.pending_sends.push_back(receipt);
                self.output_slots = frame.slots;
                self.outgoing.authored = Some(source.output);
                Ok::<_, String>(())
            })();
            if let Err(error) = result {
                // A live native mailbox can be temporarily full. Keep the same
                // prepared identity and its source lease; do not decode or replay it.
                if self.transport.as_ref().is_some_and(DmxTransportHandle::is_ready) {
                    self.outgoing.pending.push_front(pending);
                    self.outgoing.retry_at = Some(Instant::now() + Duration::from_millis(250));
                    if let Some(wake) = &self.wake {
                        wake.later(Duration::from_millis(250));
                    }
                    return;
                }
                self.set_transport_warning(ctx, &error);
                continue;
            }
            if self.outgoing.pending.is_empty() && !matches!(source.input, Input::Property(_)) {
                let current = snapshot
                    .subtree_revision(source.output.0)
                    .map(|revision| (source.output.0, revision));
                let current_text = snapshot
                    .node(source.output.0)
                    .and_then(|node| node.param_value.as_ref());
                let own_projection = matches!(current_text, Some(ParamValue::Str(text)) if self.outgoing.projection.as_ref() == Some(text));
                if current == Some(source.output) || own_projection {
                    use golden_core::edit::{Edit, EditPreconditions, NodeReadGuard};
                    let mut guards = Vec::with_capacity(7);
                    guards.push(NodeReadGuard::capture_structure(&snapshot, self.id()).unwrap());
                    guards.push(NodeReadGuard::capture(&snapshot, source.output.0).unwrap());
                    for (id, _) in source.key.configuration.iter().flatten() {
                        guards.push(NodeReadGuard::capture(&snapshot, *id).unwrap());
                    }
                    ctx.edits.push(Edit::CheckPreconditions {
                        preconditions: EditPreconditions::new(guards, vec![source.session.clone()]).unwrap(),
                    });
                    self.persist_output_slots(ctx);
                    self.outgoing.projection = Some(slots_json(&self.output_slots));
                    ctx.edits.retain_payload(ready.clone());
                }
            }
        }
        if !self.outgoing.pending.is_empty() {
            self.request_service();
        }
    }

    pub(in super::super) fn output_needs_snapshot(&self) -> bool {
        self.has_pending_output() && self.outgoing.retry_at.is_none_or(|at| Instant::now() >= at)
    }
    pub(in super::super) fn has_pending_output(&self) -> bool {
        !self.outgoing.pending.is_empty()
    }
    pub(in super::super) fn clear_output(&mut self) {
        if !self.outgoing.pending.is_empty() {
            golden_core::logerror!(format!(
                "DMX retired {} pending requests; they were not sent",
                self.outgoing.pending.len()
            ));
        }
        self.outgoing = Outgoing::default();
    }
}

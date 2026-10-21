use super::*;
use golden_io::WorkerReceive;

pub(crate) struct MidiDiscovery {
    worker: BoundedWorker<(), Result<MidiPortOptions, String>>,
}
impl MidiDiscovery {
    pub(crate) fn spawn(notifier: WorkerNotifier) -> Result<Self, String> {
        Self::spawn_using(lifetimes(), notifier, ports::available_midi_port_options)
    }
    pub(super) fn spawn_using(
        pool: &RetirementPool,
        notifier: WorkerNotifier,
        mut detect: impl FnMut() -> Result<MidiPortOptions, String> + Send + 'static,
    ) -> Result<Self, String> {
        let worker = BoundedWorker::spawn_notified(
            "midi-discovery",
            pool,
            WorkerLimits {
                command_items: 1,
                command_weight: 1,
                event_items: 1,
                event_weight: 256 * 1024,
            },
            notifier,
            move |ctx| {
                let mut previous = None;
                loop {
                    if ctx.cancellation().is_cancelled() {
                        return;
                    }
                    let result = detect().and_then(validate_catalog).map_err(diagnostic);
                    if ctx.cancellation().is_cancelled() {
                        return;
                    }
                    if previous.as_ref() != Some(&result) {
                        // A single bounded latest catalog may wait for publication. No
                        // subsequent native query or producer-driven thread is created.
                        let weight = result.as_ref().map_or_else(String::capacity, catalog_weight)
                            + std::mem::size_of::<Result<MidiPortOptions, String>>();
                        if !publish(&ctx, result.clone(), weight) {
                            return;
                        }
                        previous = Some(result);
                    }
                    match ctx.receive_until(Instant::now() + Duration::from_secs(1)) {
                        WorkerReceive::Disconnected => return,
                        WorkerReceive::Command(()) | WorkerReceive::Deadline => {}
                    }
                }
            },
        )
        .map_err(|error| format!("failed to start MIDI discovery: {error}"))?;
        Ok(Self { worker })
    }
    pub(crate) fn take(&self) -> (Option<Result<MidiPortOptions, String>>, bool) {
        let mut events = Vec::with_capacity(1);
        let drain = self.worker.drain_events(&mut events, NonZeroUsize::MIN);
        (events.pop(), drain.state == PendingDrainState::Disconnected)
    }
}
fn validate_catalog(catalog: MidiPortOptions) -> Result<MidiPortOptions, String> {
    for ports in [&catalog.inputs, &catalog.outputs] {
        if ports.capacity() > 64
            || ports
                .iter()
                .any(|p| p.label.capacity() > 512 || p.variant_id.capacity() > 1024 || p.details.capacity() > 128)
        {
            return Err("MIDI catalog exceeds 64 ports or its metadata field bounds".into());
        }
    }
    Ok(catalog)
}
fn catalog_weight(catalog: &MidiPortOptions) -> usize {
    [&catalog.inputs, &catalog.outputs]
        .into_iter()
        .map(|ports| {
            ports.capacity() * std::mem::size_of::<DiscoveredMidiPort>()
                + ports
                    .iter()
                    .map(|p| p.label.capacity() + p.variant_id.capacity() + p.details.capacity())
                    .sum::<usize>()
        })
        .sum()
}

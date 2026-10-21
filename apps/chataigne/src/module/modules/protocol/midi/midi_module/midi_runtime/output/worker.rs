use super::*;
use golden_io::WorkerReceive;

pub(super) fn run<G: OutputConnection>(
    ctx: WorkerContext<AdmittedRequest, MidiOutputEvent>,
    config: MidiOutputConfig,
    state: Arc<AtomicBool>,
    status: WorkerNotifier,
    open: impl FnOnce(&MidiOutputConfig) -> Result<G, String>,
) {
    let mut connection = match open(&config) {
        Ok(connection) => connection,
        Err(error) => {
            emit(&ctx, MidiOutputEvent::Error(diagnostic(error)));
            return;
        }
    };
    let _readiness = Readiness {
        state: state.clone(),
        notifier: status.clone(),
    };
    if ctx.cancellation().is_cancelled() {
        return;
    }
    state.store(true, Ordering::Release);
    if !emit(&ctx, MidiOutputEvent::Ready) {
        return;
    }
    let mut schedule = schedule::Schedule::default();
    let mut check_at = Instant::now() + Duration::from_secs(1);
    let mut failed = false;
    let mut order = 0u64;
    let mut pending = None;
    loop {
        if ctx.cancellation().is_cancelled() {
            break;
        }
        let mut requests = Vec::with_capacity(1);
        let drain = if let Some(request) = pending.take() {
            requests.push(request);
            golden_io::PendingDrain {
                received: 1,
                state: PendingDrainState::BudgetExhausted,
            }
        } else {
            ctx.drain_commands(&mut requests, NonZeroUsize::MIN)
        };
        for request in requests {
            if failed {
                if !emit(
                    &ctx,
                    MidiOutputEvent::Error(
                        "accepted MIDI request was not prepared or sent after output failure".into(),
                    ),
                ) {
                    return;
                }
                continue;
            }
            let result = order
                .checked_add(1)
                .ok_or_else(|| "MIDI output sequence exhausted".to_string())
                .and_then(|next| {
                    order = next;
                    schedule::Batch::prepare(request, next)
                });
            match result {
                Ok(batch) => schedule.push(batch),
                Err(error) => {
                    if !emit(&ctx, MidiOutputEvent::Error(diagnostic(error))) {
                        return;
                    }
                }
            }
        }
        if !failed && Instant::now() >= check_at {
            match connection.is_present() {
                Ok(true) => check_at = Instant::now() + Duration::from_secs(1),
                result => {
                    fail(&ctx, &state, &status);
                    failed = true;
                    if !emit(
                        &ctx,
                        MidiOutputEvent::Error(diagnostic(
                            result
                                .err()
                                .unwrap_or_else(|| "selected MIDI output disappeared".into()),
                        )),
                    ) {
                        break;
                    }
                }
            }
        }
        for _ in 0..16 {
            let Some((event, native_failed)) = schedule.service(&mut connection, failed) else {
                break;
            };
            if native_failed {
                fail(&ctx, &state, &status);
                failed = true;
            }
            if !emit(&ctx, event) {
                return;
            }
        }
        if drain.state == PendingDrainState::Disconnected && schedule.is_empty() {
            break;
        }
        if drain.state == PendingDrainState::BudgetExhausted
            || failed && !schedule.is_empty()
            || schedule.deadline().is_some_and(|at| at <= Instant::now())
        {
            continue;
        }
        let deadline = schedule.deadline().map_or(check_at, |at| at.min(check_at));
        match ctx.receive_until(deadline) {
            WorkerReceive::Command(request) => pending = Some(request),
            WorkerReceive::Deadline => {}
            WorkerReceive::Disconnected => {
                // Graceful closure can leave scheduled follow-ups. Only this
                // worker waits for their deadline; explicit cancellation exits.
                if ctx.cancellation().is_cancelled() || schedule.is_empty() {
                    break;
                }
                if ctx
                    .cancellation()
                    .wait(deadline.saturating_duration_since(Instant::now()))
                {
                    break;
                }
            }
        }
    }
    state.store(false, Ordering::Release);
    status.notify();
    drop(schedule);
    drop(connection);
}
fn emit(ctx: &WorkerContext<AdmittedRequest, MidiOutputEvent>, event: MidiOutputEvent) -> bool {
    let weight = event.weight();
    publish(ctx, event, weight)
}
fn fail(ctx: &WorkerContext<AdmittedRequest, MidiOutputEvent>, state: &AtomicBool, status: &WorkerNotifier) {
    state.store(false, Ordering::Release);
    status.notify();
    ctx.close_command_admission();
}

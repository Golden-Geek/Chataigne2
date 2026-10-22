use super::*;
use golden_io::{WorkerContext, WorkerReceive};
use std::time::{Duration, Instant};

struct Running {
    shared: Arc<Shared>,
    source: Option<RevisionSource>,
}
impl Drop for Running {
    fn drop(&mut self) {
        self.source = None;
        self.shared.running.store(false, Ordering::Release);
        self.shared.wake.notify();
    }
}
pub(super) fn run<P: Provider>(
    ctx: WorkerContext<Work, Status>,
    shared: Arc<Shared>,
    native: RevisionSource,
    fence: Fence,
    input: PayloadPool,
    output: PayloadPool,
    open: impl FnOnce() -> Result<P, String>,
) {
    let running = Running {
        shared: shared.clone(),
        source: Some(native),
    };
    let mut provider = match open() {
        Ok(provider) => provider,
        Err(error) => {
            let _ = ctx.publish_weighted(Status::Error(diagnostic(error)), 2048);
            return;
        }
    };
    // Source retirement and the stopped flag precede the provider's destructor.
    let _running = running;
    if ctx.cancellation().is_cancelled() {
        return;
    }
    if ctx.publish_weighted(Status::Ready, 128).is_err() {
        return;
    }
    let mut next_sample = Instant::now();
    let mut pending = None;
    while !ctx.cancellation().is_cancelled() {
        let mut commands = Vec::with_capacity(8);
        if let Some(command) = pending.take() {
            commands.push(command);
        }
        if commands.len() < 8 {
            let remaining = 8 - commands.len();
            ctx.drain_commands(&mut commands, NonZeroUsize::new(remaining).unwrap());
        }
        for command in commands {
            if ctx.cancellation().is_cancelled() {
                return;
            }
            command.run(&mut provider, &output);
        }
        if ctx.cancellation().is_cancelled() {
            return;
        }
        if Instant::now() >= next_sample {
            next_sample = Instant::now() + Duration::from_secs(1);
            let reservation = match input.try_reserve(WEIGHT) {
                Ok(reservation) => reservation,
                Err(_) => {
                    let _ = ctx.publish_weighted(
                        Status::Warning("OS metrics capacity exhausted; sample withheld".into()),
                        2048,
                    );
                    continue;
                }
            };
            let snapshot = match provider.sample() {
                Ok(snapshot) if snapshot.os_version.capacity() <= 4096 && snapshot.architecture.capacity() <= 4096 => {
                    snapshot
                }
                Ok(_) => {
                    let _ = ctx.publish_weighted(
                        Status::Error("OS information exceeds its 4096-byte field bounds".into()),
                        2048,
                    );
                    return;
                }
                Err(error) => {
                    let _ = ctx.publish_weighted(Status::Error(diagnostic(error)), 2048);
                    return;
                }
            };
            let result = (|| {
                let callback = NativeCallbackPayload::prepare(
                    super::super::SYSTEM_STATS_UPDATED_CALLBACK,
                    vec![super::super::metrics_json(&snapshot)],
                )?;
                reservation
                    .retain(
                        Metrics {
                            snapshot,
                            callback,
                            fence: fence.clone(),
                        },
                        WEIGHT,
                    )
                    .map_err(|_| "OS metrics reservation failed".into())
            })();
            match result {
                Ok(snapshot) => {
                    *shared.latest.lock().unwrap_or_else(|poison| poison.into_inner()) = Some(snapshot);
                    shared.wake.notify();
                }
                Err(error) => {
                    let _ = ctx.publish_weighted(Status::Warning(diagnostic(error)), 2048);
                }
            }
        }
        match ctx.receive_until(next_sample) {
            WorkerReceive::Command(command) => pending = Some(command),
            WorkerReceive::Deadline => {}
            WorkerReceive::Disconnected => return,
        }
    }
}

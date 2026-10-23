use super::*;
use golden_io::{PendingDrainState, ReconnectBackoff, WorkerContext, WorkerReceive};
use std::time::{Duration, Instant};

pub(super) trait Provider: 'static {
    fn connect(&mut self, config: &ButtplugTransportConfig) -> Result<Vec<ButtplugWorkerEvent>, String>;
    fn poll(&mut self) -> Result<Vec<ButtplugWorkerEvent>, String>;
    fn execute(&mut self, command: &ButtplugWorkerCommand) -> Result<String, String>;
    fn disconnect(&mut self);
}
// Native cleanup sends stop-all. Keep the module's connection lease until that
// cleanup finishes, so it cannot stop output from a replacement connection.
pub(super) struct Exclusive<P> {
    provider: P,
    _permit: golden_io::RetirementPermit,
}
impl<P> Exclusive<P> {
    pub(super) fn new(provider: P, permit: golden_io::RetirementPermit) -> Self {
        Self {
            provider,
            _permit: permit,
        }
    }
}
impl<P: Provider> Provider for Exclusive<P> {
    fn connect(&mut self, config: &ButtplugTransportConfig) -> Result<Vec<ButtplugWorkerEvent>, String> {
        self.provider.connect(config)
    }
    fn poll(&mut self) -> Result<Vec<ButtplugWorkerEvent>, String> {
        self.provider.poll()
    }
    fn execute(&mut self, command: &ButtplugWorkerCommand) -> Result<String, String> {
        self.provider.execute(command)
    }
    fn disconnect(&mut self) {
        self.provider.disconnect();
    }
}
pub(super) type Context = WorkerContext<AdmittedCommand, RetainedPayload<Publication>>;
struct Stopped {
    shared: Arc<Shared>,
    life: Option<NativeLife>,
}
impl Drop for Stopped {
    fn drop(&mut self) {
        self.life = None;
        self.shared.connected.store(false, Ordering::Release);
        self.shared.running.store(false, Ordering::Release);
        self.shared.notifier.notify();
    }
}
// Field destruction announces termination before the native provider can block in cleanup.
struct Owned<P> {
    _stopped: Stopped,
    context: Context,
    provider: P,
}

pub(super) fn run<P: Provider>(
    ctx: Context,
    config: RetainedPayload<ButtplugTransportConfig>,
    events: PayloadPool,
    shared: Arc<Shared>,
    life: NativeLife,
    open: impl FnOnce() -> Result<P, String>,
) {
    let stopped = Stopped {
        shared: shared.clone(),
        life: Some(life),
    };
    if ctx.cancellation().is_cancelled() {
        return;
    }
    let provider = match open() {
        Ok(provider) => provider,
        Err(error) => {
            publication::Publisher::new(&ctx, &events, &shared)
                .publish(ButtplugWorkerEvent::Error(admission::diagnostic(error)));
            return;
        }
    };
    let mut owned = Owned {
        _stopped: stopped,
        context: ctx,
        provider,
    };
    let ctx = &owned.context;
    let life = owned._stopped.life.as_ref().unwrap();
    let mut publisher = publication::Publisher::new(ctx, &events, &shared);
    let mut backoff = ReconnectBackoff::new(Duration::from_millis(250), Duration::from_secs(5));
    let mut next_connect = Instant::now();
    let mut active = false;
    let mut waiting = None;
    while !ctx.cancellation().is_cancelled() {
        if !active && Instant::now() >= next_connect {
            match owned.provider.connect(&config) {
                Ok(initial) => {
                    if ctx.cancellation().is_cancelled() {
                        break;
                    }
                    life.session.advance();
                    shared.connected.store(true, Ordering::Release);
                    active = true;
                    backoff.reset();
                    if !publisher.all(initial) {
                        break;
                    }
                }
                Err(error) => {
                    shared.connected.store(false, Ordering::Release);
                    life.session.advance();
                    next_connect = backoff.schedule(Instant::now());
                    if !publisher.recovering(error) {
                        break;
                    }
                    owned.provider.disconnect();
                }
            }
        }
        if !publisher.selection_changed() {
            break;
        }
        let mut commands = Vec::with_capacity(8);
        if let Some(command) = waiting.take() {
            commands.push(command);
        }
        let remaining = NonZeroUsize::new(8 - commands.len()).unwrap();
        let drain = ctx.drain_commands(&mut commands, remaining);
        for command in commands {
            if ctx.cancellation().is_cancelled() || !active || !command.source.live() {
                command.finish(CommandStatus::NotSent);
                continue;
            }
            let request = match command.source.prepare() {
                Ok(request) if command.source.live() => request,
                Ok(_) => {
                    command.finish(CommandStatus::NotSent);
                    continue;
                }
                Err(error) => {
                    command.finish(CommandStatus::Completed(Err(admission::diagnostic(error))));
                    continue;
                }
            };
            command.started();
            let result = owned
                .provider
                .execute(&request)
                .map(admission::diagnostic)
                .map_err(admission::diagnostic);
            let scanning = match (&request, &result) {
                (ButtplugWorkerCommand::StartScanning, Ok(_)) => Some(true),
                (ButtplugWorkerCommand::StopScanning, Ok(_)) => Some(false),
                _ => None,
            };
            command.finish(CommandStatus::Completed(result));
            if let Some(scanning) = scanning {
                if !publisher.publish(ButtplugWorkerEvent::Scanning(scanning)) {
                    return;
                }
            }
        }
        if drain.state == PendingDrainState::Disconnected || ctx.cancellation().is_cancelled() {
            break;
        }
        if active {
            match owned.provider.poll() {
                Ok(input) => {
                    if !publisher.all(input) {
                        break;
                    }
                }
                Err(error) => {
                    shared.connected.store(false, Ordering::Release);
                    life.session.advance();
                    active = false;
                    next_connect = backoff.schedule(Instant::now());
                    if !publisher.recovering(error) {
                        break;
                    }
                    owned.provider.disconnect();
                }
            }
        } else {
            match ctx.receive_until(next_connect) {
                WorkerReceive::Command(command) => waiting = Some(command),
                WorkerReceive::Deadline => {}
                WorkerReceive::Disconnected => break,
            }
        }
    }
}

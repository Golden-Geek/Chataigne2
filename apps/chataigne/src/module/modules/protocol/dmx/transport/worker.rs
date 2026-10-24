use super::*;
use crate::app::module::script_api::NativeCallbackPayload;
use golden_io::{WorkerContext, WorkerReceive};
use std::time::{Duration, Instant};

const TURN: usize = 8;
const RECEIVE_TURN: usize = 64;
const RECEIVE_BUDGET: Duration = Duration::from_millis(2);
const RECEIVE_WAIT: Duration = Duration::from_millis(5);

pub(super) trait Endpoint: Send + 'static {
    fn send(&mut self, frame: &DmxFrame) -> Result<(), String>;
    fn receive(&mut self) -> Result<ReceivePoll, String>;
    fn command_at(&self) -> Option<Instant> {
        None
    }
    fn deadline(&self) -> Option<Instant> {
        None
    }
    fn service(&mut self) -> Result<(), String> {
        Ok(())
    }
    fn shutdown(&mut self) {}
}
impl Endpoint for native::WorkerEndpoint {
    fn send(&mut self, frame: &DmxFrame) -> Result<(), String> {
        self.send(frame)
    }
    fn receive(&mut self) -> Result<ReceivePoll, String> {
        self.receive()
    }
    fn command_at(&self) -> Option<Instant> {
        Some(self.command_at())
    }
    fn deadline(&self) -> Option<Instant> {
        self.deadline()
    }
    fn service(&mut self) -> Result<(), String> {
        self.service()
    }
    fn shutdown(&mut self) {
        self.shutdown();
    }
}
struct Session<E: Endpoint>(E);
impl<E: Endpoint> Drop for Session<E> {
    fn drop(&mut self) {
        self.0.shutdown();
    }
}
struct Running(Arc<Shared>);
impl Drop for Running {
    fn drop(&mut self) {
        self.0.ready.store(false, Ordering::Release);
        self.0.running.store(false, Ordering::Release);
        self.0.wake.notify();
    }
}
pub(super) fn run<E: Endpoint>(
    ctx: WorkerContext<Output, DmxWorkerEvent>,
    config: DmxTransportConfig,
    shared: Arc<Shared>,
    input: PayloadPool,
    source: golden_io::RevisionGuard,
    open: impl FnOnce(&DmxTransportConfig) -> Result<E, String>,
) {
    let running = Running(shared.clone());
    let endpoint = match open(&config) {
        Ok(endpoint) => endpoint,
        Err(error) => {
            publish(&ctx, DmxWorkerEvent::Error(diagnostic(error)));
            return;
        }
    };
    let mut session = Session(endpoint);
    // Publish termination before shutdown/destruction can block, retaining the
    // original worker slot through both. Setup failure is guarded as well.
    let _running = running;
    if ctx.cancellation().is_cancelled() {
        return;
    }
    shared.ready.store(true, Ordering::Release);
    if !publish(&ctx, DmxWorkerEvent::Ready) {
        return;
    }
    let mut commands = std::collections::VecDeque::<Output>::with_capacity(TURN);
    let mut pressured = false;
    let mut invalid_reported = false;
    while !ctx.cancellation().is_cancelled() {
        let remaining = TURN - commands.len();
        if remaining != 0 {
            let mut admitted = Vec::with_capacity(remaining);
            ctx.drain_commands(&mut admitted, NonZeroUsize::new(remaining).unwrap());
            commands.extend(admitted);
        }
        while let Some(command) = commands.front() {
            if ctx.cancellation().is_cancelled() {
                return;
            }
            if !command.is_live() {
                commands.pop_front().unwrap().reject();
                continue;
            }
            if session.0.command_at().is_some_and(|deadline| Instant::now() < deadline) {
                break;
            }
            let command = commands.pop_front().unwrap();
            let result = session.0.send(&command.frame);
            let error = result.as_ref().err().cloned();
            command.complete(result);
            if let Some(error) = error {
                publish(&ctx, DmxWorkerEvent::Error(diagnostic(error)));
                return;
            }
        }
        if ctx.cancellation().is_cancelled() {
            return;
        }
        if let Err(error) = session.0.service() {
            publish(&ctx, DmxWorkerEvent::Error(diagnostic(error)));
            return;
        }
        let mut receive_may_have_more = false;
        if config.receive_enabled {
            let receive_started = Instant::now();
            for _ in 0..RECEIVE_TURN {
                if ctx.cancellation().is_cancelled() {
                    return;
                }
                match session.0.receive() {
                    Ok(ReceivePoll::Empty) => {
                        receive_may_have_more = false;
                        break;
                    }
                    Ok(ReceivePoll::Ignored) => {}
                    Ok(ReceivePoll::Invalid(message)) => {
                        // An untrusted sender cannot fill the status mailbox or force reconnects.
                        if !invalid_reported {
                            if !publish(&ctx, DmxWorkerEvent::Warning(message.into())) {
                                return;
                            }
                            invalid_reported = true;
                        }
                    }
                    Ok(ReceivePoll::Frame(frame)) => match prepare_input(frame, &config, source.clone(), &input) {
                        Ok(frame) => {
                            let mut latest = shared.latest.lock().unwrap_or_else(|poison| poison.into_inner());
                            if latest.replace(frame).is_some() {
                                shared.replaced.fetch_add(1, Ordering::Relaxed);
                            }
                            drop(latest);
                            shared.wake.notify();
                            pressured = false;
                        }
                        Err(error) => {
                            shared.replaced.fetch_add(1, Ordering::Relaxed);
                            if !pressured {
                                if !publish(&ctx, DmxWorkerEvent::Warning(diagnostic(error))) {
                                    return;
                                }
                                pressured = true;
                            }
                        }
                    },
                    Err(error) => {
                        publish(&ctx, DmxWorkerEvent::Error(diagnostic(error)));
                        return;
                    }
                }
                receive_may_have_more = true;
                if receive_started.elapsed() >= RECEIVE_BUDGET {
                    break;
                }
            }
        }
        let wait = if receive_may_have_more {
            Duration::ZERO
        } else if !config.receive_enabled {
            Duration::from_secs(60)
        } else {
            RECEIVE_WAIT
        };
        let mut deadline = Instant::now() + wait;
        if let Some(output_deadline) = session.0.deadline() {
            deadline = deadline.min(output_deadline);
        }
        if !commands.is_empty() {
            if let Some(command_at) = session.0.command_at() {
                deadline = deadline.min(command_at);
            }
            // Keep this bounded FIFO prefix and its original receipt/source leases intact.
            // Do not dequeue more work merely to wait for the protocol's send clock.
            ctx.cancellation()
                .wait(deadline.saturating_duration_since(Instant::now()));
            continue;
        }
        match ctx.receive_until(deadline) {
            WorkerReceive::Command(command) => commands.push_back(command),
            WorkerReceive::Deadline => {}
            WorkerReceive::Disconnected => break,
        }
    }
}
fn publish(ctx: &WorkerContext<Output, DmxWorkerEvent>, event: DmxWorkerEvent) -> bool {
    let weight = match &event {
        DmxWorkerEvent::Warning(text) | DmxWorkerEvent::Error(text) => 128 + text.capacity(),
        _ => 128,
    };
    ctx.publish_weighted(event, weight).is_ok()
}
fn prepare_input(
    frame: DmxFrame,
    config: &DmxTransportConfig,
    source: golden_io::RevisionGuard,
    input: &PayloadPool,
) -> Result<RetainedPayload<ReceivedFrame>, String> {
    validate_frame(&frame, config.protocol)?;
    let reservation = input
        .try_reserve(FRAME_WEIGHT)
        .map_err(|_| "DMX incoming payload capacity exhausted")?;
    let json = super::super::frame::slots_json(&frame.slots);
    let callback = NativeCallbackPayload::prepare(
        "dmxFrameReceived",
        vec![
            serde_json::json!(frame.universe),
            serde_json::json!(&frame.slots),
            serde_json::json!({"protocol": config.protocol.label(), "sequence": frame.sequence, "priority": frame.priority}),
        ],
    )?;
    reservation
        .retain(
            ReceivedFrame {
                frame,
                json,
                callback,
                source,
            },
            FRAME_WEIGHT,
        )
        .map_err(|_| "DMX incoming reservation failed".into())
}

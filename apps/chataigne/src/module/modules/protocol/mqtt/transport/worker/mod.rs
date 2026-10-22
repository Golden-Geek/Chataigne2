use super::{publications::Publications, *};
use golden_io::{PendingDrainState, ReconnectBackoff, RetainedPayload, WorkerContext};
use rumqttc::{AsyncClient, ConnectionError, Event, EventLoop, MqttOptions, Outgoing, Packet};
use std::{
    future::Future,
    net::{SocketAddr, ToSocketAddrs},
    num::NonZeroUsize,
    pin::Pin,
    time::{Duration, Instant},
};

const POLL: Duration = Duration::from_millis(10);
const INFLIGHT: u16 = 16;
const COMMAND_TURN: NonZeroUsize = NonZeroUsize::new(8).unwrap();

pub(super) enum BrokerPoll {
    Event(Event),
    Idle,
    Closed,
}
pub(super) trait Broker: Send + 'static {
    fn prepare(&mut self) -> Result<(), String> {
        Ok(())
    }
    fn poll(&mut self) -> Result<BrokerPoll, String>;
    fn publish(&mut self, request: &MqttPublishRequest) -> bool;
    fn subscribe(&mut self, subscription: &MqttSubscriptionConfig) -> bool;
    fn buffered(&mut self) -> Option<Event> {
        None
    }
}

pub(super) struct NativeBroker {
    client: AsyncClient,
    eventloop: Option<EventLoop>,
    polling: Option<ProtocolPoll>,
    runtime: tokio::runtime::Runtime,
    config: MqttTransportConfig,
    resolver: Resolver,
    address_cursor: usize,
}
type ProtocolPoll = Pin<Box<dyn Future<Output = (EventLoop, Result<Event, ConnectionError>)> + Send>>;
type Resolver = Box<dyn FnMut(&str, u16) -> Result<Vec<SocketAddr>, String> + Send>;
impl NativeBroker {
    pub(super) fn open(config: &MqttTransportConfig) -> Self {
        Self::with_resolver(config, |host, port| {
            (host, port)
                .to_socket_addrs()
                .map(|addresses| addresses.take(16).collect())
                .map_err(|error| error.to_string())
        })
    }
    pub(super) fn with_resolver(
        config: &MqttTransportConfig,
        resolver: impl FnMut(&str, u16) -> Result<Vec<SocketAddr>, String> + Send + 'static,
    ) -> Self {
        // No connect is polled until the synchronous resolver has completed and
        // the worker has checked cancellation. Runtime DNS cannot accumulate
        // timed-out blocking lookups behind repeated reconnect attempts.
        let options = Self::options(config, "0.0.0.0".into());
        let (client, mut eventloop) = AsyncClient::new(options, 64);
        let mut network = eventloop.network_options();
        network.set_connection_timeout(1);
        eventloop.set_network_options(network);
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .max_blocking_threads(1)
            .build()
            .expect("MQTT native runtime setup failed");
        Self {
            client,
            eventloop: Some(eventloop),
            polling: None,
            runtime,
            config: config.clone(),
            resolver: Box::new(resolver),
            address_cursor: 0,
        }
    }
    fn options(config: &MqttTransportConfig, host: String) -> MqttOptions {
        let mut options = MqttOptions::new(config.client_id.clone(), host, config.remote_port);
        options
            .set_max_packet_size(MAX_PACKET, MAX_PACKET)
            .set_inflight(INFLIGHT)
            .set_clean_session(config.clean_session)
            .set_keep_alive(Duration::from_secs(config.keep_alive_secs));
        if let Some(credentials) = &config.credentials {
            options.set_credentials(credentials.username.clone(), credentials.password.clone());
        }
        options
    }
}
impl Broker for NativeBroker {
    fn prepare(&mut self) -> Result<(), String> {
        if let Some(eventloop) = self.eventloop.as_mut().filter(|eventloop| eventloop.network.is_none()) {
            let addresses = (self.resolver)(&self.config.remote_host, self.config.remote_port)?;
            if addresses.is_empty() {
                return Err("MQTT host resolved to no addresses".into());
            }
            let address = addresses[self.address_cursor % addresses.len()];
            self.address_cursor = (self.address_cursor + 1) % addresses.len();
            let host = match address {
                SocketAddr::V4(address) => address.ip().to_string(),
                SocketAddr::V6(address) => format!("[{}]", address.ip()),
            };
            eventloop.mqtt_options = Self::options(&self.config, host);
        }
        Ok(())
    }
    fn poll(&mut self) -> Result<BrokerPoll, String> {
        if self.polling.is_none() {
            let mut eventloop = self.eventloop.take().expect("idle protocol owns its event loop");
            self.polling = Some(Box::pin(async move {
                let event = eventloop.poll().await;
                (eventloop, event)
            }));
        }
        // Timeout only the wait, retaining the owned poll future. Cancelling a
        // fresh EventLoop::poll on every short turn could restart connects or
        // leave socket flushes incomplete while accepting more QoS-0 output.
        let future = self.polling.as_mut().unwrap();
        let result = self
            .runtime
            .block_on(async { tokio::time::timeout(POLL, future).await });
        let Ok((mut eventloop, event)) = result else {
            return Ok(BrokerPoll::Idle);
        };
        self.polling = None;
        if let Ok(Event::Incoming(Packet::ConnAck(ack))) = &event {
            restore_session(&mut eventloop.state, ack.session_present);
        }
        self.eventloop = Some(eventloop);
        match event {
            Ok(event) => Ok(BrokerPoll::Event(event)),
            Err(ConnectionError::RequestsDone) => Ok(BrokerPoll::Closed),
            Err(error) => Err(error.to_string()),
        }
    }
    fn publish(&mut self, request: &MqttPublishRequest) -> bool {
        self.client
            .try_publish(
                request.topic.clone(),
                request.qos.to_rumqttc(),
                request.retain,
                request.payload.clone(),
            )
            .is_ok()
    }
    fn subscribe(&mut self, subscription: &MqttSubscriptionConfig) -> bool {
        self.client
            .try_subscribe(subscription.topic_filter.clone(), subscription.qos.to_rumqttc())
            .is_ok()
    }
    fn buffered(&mut self) -> Option<Event> {
        self.eventloop
            .as_mut()
            .and_then(|eventloop| eventloop.state.events.pop_front())
    }
}

// rumqttc clears its pending list for a new session, but its public state
// also contains a collision and buffered events. Those identities cannot survive
// a broker session reset. A resumed session keeps the original protocol state.
pub(super) fn restore_session(state: &mut rumqttc::MqttState, present: bool) {
    if !present {
        *state = rumqttc::MqttState::new(INFLIGHT, false);
    }
}

/// Library-held payloads must be destroyed before their tracking owners release
/// lifetime quotas, including during panic or a stalled native destructor.
struct Session<B: Broker> {
    broker: Option<B>,
    publications: Publications,
}
impl<B: Broker> Drop for Session<B> {
    fn drop(&mut self) {
        drop(self.broker.take());
        self.publications.session_lost();
    }
}
struct Ready {
    connected: Arc<AtomicBool>,
    running: Arc<AtomicBool>,
    notifier: WorkerNotifier,
}
impl Drop for Ready {
    fn drop(&mut self) {
        self.connected.store(false, Ordering::Release);
        self.running.store(false, Ordering::Release);
        self.notifier.notify();
    }
}

pub(super) fn run<B: Broker>(
    ctx: WorkerContext<AdmittedPublish, MqttWorkerEvent>,
    config: RetainedPayload<MqttTransportConfig>,
    connected: Arc<AtomicBool>,
    running: Arc<AtomicBool>,
    input: PayloadPool,
    notifier: WorkerNotifier,
    open: impl FnOnce(&MqttTransportConfig) -> B,
) {
    let ready = Ready {
        connected: connected.clone(),
        running,
        notifier,
    };
    let mut session = Session {
        broker: Some(open(&config)),
        publications: Publications::default(),
    };
    // Also cover setup panics, then announce termination before native cleanup
    // can block. Cleanup still owns its original worker and payload reservations.
    let _ready = ready;
    let mut pending = std::collections::VecDeque::new();
    let mut subscriptions = 0;
    let mut backoff = ReconnectBackoff::new(Duration::from_millis(250), Duration::from_secs(8));
    let mut last_status = None;
    while !ctx.cancellation().is_cancelled() {
        if connected.load(Ordering::Acquire) {
            // Finish pending subscription admission before accepting more output.
            for _ in 0..4 {
                let Some(subscription) = config.subscriptions.get(subscriptions) else {
                    break;
                };
                if !session.broker.as_mut().unwrap().subscribe(subscription) {
                    break;
                }
                subscriptions += 1;
            }
            if pending.is_empty() {
                let mut commands = Vec::new();
                let drain = ctx.drain_commands(&mut commands, COMMAND_TURN);
                pending.extend(commands);
                if drain.state == PendingDrainState::Disconnected && pending.is_empty() {
                    break;
                }
            }
            for _ in 0..COMMAND_TURN.get() {
                let Some(command) = pending.front_mut() else { break };
                if let Err(error) = command.prepare() {
                    pending.pop_front().unwrap().reject(error);
                    continue;
                }
                if !session.broker.as_mut().unwrap().publish(command.request()) {
                    break;
                }
                session.publications.queued(pending.pop_front().unwrap());
            }
        }
        let poll = match session.broker.as_mut().unwrap().prepare() {
            Ok(()) if ctx.cancellation().is_cancelled() => break,
            Ok(()) => session.broker.as_mut().unwrap().poll(),
            Err(error) => Err(error),
        };
        match poll {
            Ok(BrokerPoll::Event(event)) => {
                if matches!(event, Event::Incoming(Packet::ConnAck(_))) {
                    subscriptions = 0;
                    backoff.reset();
                }
                if !deliver(
                    &ctx,
                    &mut session.publications,
                    &connected,
                    &input,
                    &mut last_status,
                    event,
                ) {
                    break;
                }
            }
            Ok(BrokerPoll::Idle) => {}
            Ok(BrokerPoll::Closed) => break,
            Err(error) => {
                connected.store(false, Ordering::Release);
                // rumqttc may have applied state transitions before a failed flush.
                // Account for those events before reconnection can clear a session.
                while let Some(event) = session.broker.as_mut().unwrap().buffered() {
                    if !deliver(
                        &ctx,
                        &mut session.publications,
                        &connected,
                        &input,
                        &mut last_status,
                        event,
                    ) {
                        return;
                    }
                }
                if !status(
                    &ctx,
                    &mut last_status,
                    MqttConnectionStatus::Recovering {
                        message: admission::diagnostic(error),
                    },
                ) {
                    break;
                }
                let deadline = backoff.schedule(Instant::now());
                if ctx
                    .cancellation()
                    .wait(deadline.saturating_duration_since(Instant::now()))
                {
                    break;
                }
            }
        }
    }
}

fn deliver(
    ctx: &WorkerContext<AdmittedPublish, MqttWorkerEvent>,
    publications: &mut Publications,
    connected: &AtomicBool,
    input: &PayloadPool,
    last_status: &mut Option<MqttConnectionStatus>,
    event: Event,
) -> bool {
    match event {
        Event::Incoming(Packet::ConnAck(ack)) => {
            if !ack.session_present {
                publications.session_lost();
            }
            connected.store(true, Ordering::Release);
            status(ctx, last_status, MqttConnectionStatus::Connected)
        }
        Event::Outgoing(Outgoing::Publish(id)) => {
            publications.dispatched(id);
            true
        }
        Event::Incoming(Packet::PubAck(ack)) => {
            publications.acknowledged(ack.pkid);
            true
        }
        Event::Incoming(Packet::PubComp(ack)) => {
            publications.acknowledged(ack.pkid);
            true
        }
        Event::Incoming(Packet::Publish(publish)) => {
            let Some(bytes) = admission::incoming_bytes(publish.topic.capacity(), publish.payload.len()) else {
                return error(ctx, "MQTT incoming publish exceeds topic/packet bounds");
            };
            let reservation = loop {
                match input.try_reserve(bytes) {
                    Ok(reservation) => break reservation,
                    Err(_) if ctx.cancellation().wait(POLL) => return false,
                    Err(_) => {}
                }
            };
            let message = MqttReceivedPublish {
                topic: publish.topic,
                payload: publish.payload.to_vec(),
                qos: match publish.qos {
                    rumqttc::QoS::AtMostOnce => MqttQos::AtMost,
                    rumqttc::QoS::AtLeastOnce => MqttQos::AtLeast,
                    rumqttc::QoS::ExactlyOnce => MqttQos::Exactly,
                },
                retain: publish.retain,
            };
            match reservation.retain(message, bytes) {
                Ok(message) => ctx.publish_weighted(MqttWorkerEvent::Publish(message), bytes).is_ok(),
                Err(_) => error(ctx, "MQTT incoming lifetime reservation failed"),
            }
        }
        Event::Incoming(Packet::Disconnect) => {
            connected.store(false, Ordering::Release);
            status(
                ctx,
                last_status,
                MqttConnectionStatus::Recovering {
                    message: "MQTT broker disconnected".into(),
                },
            )
        }
        _ => true,
    }
}
use crate::app::module::common::mqtt::MqttQos;

fn status(
    ctx: &WorkerContext<AdmittedPublish, MqttWorkerEvent>,
    last: &mut Option<MqttConnectionStatus>,
    status: MqttConnectionStatus,
) -> bool {
    if last.as_ref() == Some(&status) {
        return true;
    }
    *last = Some(status.clone());
    let weight = match &status {
        MqttConnectionStatus::Connected => 64,
        MqttConnectionStatus::Recovering { message } => 64 + message.capacity(),
    };
    ctx.publish_weighted(MqttWorkerEvent::Status(status), weight).is_ok()
}
fn error(ctx: &WorkerContext<AdmittedPublish, MqttWorkerEvent>, message: &str) -> bool {
    ctx.publish_weighted(MqttWorkerEvent::Error(message.into()), 64 + message.len())
        .is_ok()
}

#[cfg(test)]
mod tests;

use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender, SyncSender, TrySendError},
        Arc,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use rumqttc::{Client, Event as MqttEvent, MqttOptions, Packet, RecvTimeoutError};

use crate::app::module::common::mqtt::{MqttPublishRequest, MqttQos};

const MQTT_REQUEST_CHANNEL_CAPACITY: usize = 100;
const MQTT_MAX_PACKET_SIZE: usize = 1024 * 1024;
const MQTT_WORKER_POLL_INTERVAL: Duration = Duration::from_millis(10);
const MQTT_CONNECT_POLL_TIMEOUT: Duration = Duration::from_secs(1);
const MQTT_CONNECT_TIMEOUT_SECS: u64 = 1;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MqttTransportConfig {
    pub remote_host: String,
    pub remote_port: u16,
    pub client_id: String,
    pub credentials: Option<MqttCredentials>,
    pub clean_session: bool,
    pub keep_alive_secs: u64,
    pub subscriptions: Vec<MqttSubscriptionConfig>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MqttCredentials {
    pub username: String,
    pub password: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MqttSubscriptionConfig {
    pub topic_filter: String,
    pub qos: MqttQos,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MqttReceivedPublish {
    pub topic: String,
    pub payload: Vec<u8>,
    pub qos: MqttQos,
    pub retain: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum MqttConnectionStatus {
    Connected,
    Recovering { message: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum MqttWorkerEvent {
    Publish(MqttReceivedPublish),
    Status(MqttConnectionStatus),
    Error(String),
}

pub(crate) struct MqttTransportHandle {
    command_tx: SyncSender<MqttWorkerCommand>,
    event_rx: Receiver<MqttWorkerEvent>,
    connected: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl MqttTransportHandle {
    pub(crate) fn spawn(config: MqttTransportConfig) -> Result<Self, String> {
        let (command_tx, command_rx) = mpsc::sync_channel(MQTT_REQUEST_CHANNEL_CAPACITY);
        let (event_tx, event_rx) = mpsc::channel();
        let connected = Arc::new(AtomicBool::new(false));
        let worker_connected = Arc::clone(&connected);
        let thread_name = format!("mqtt-{}-{}", config.remote_host, config.remote_port);

        let worker = thread::Builder::new()
            .name(thread_name)
            .spawn(move || worker_loop(config, command_rx, event_tx, worker_connected))
            .map_err(|error| format!("failed to start MQTT worker thread: {error}"))?;

        Ok(Self {
            command_tx,
            event_rx,
            connected,
            worker: Some(worker),
        })
    }

    pub(crate) fn send(&self, request: MqttPublishRequest) -> Result<(), String> {
        if !self.connected.load(Ordering::Acquire) {
            return Err("MQTT transport is not connected".to_string());
        }

        self.command_tx
            .try_send(MqttWorkerCommand::Publish(request))
            .map_err(|error| match error {
                TrySendError::Full(_) => "MQTT request queue is full".to_string(),
                TrySendError::Disconnected(_) => "MQTT worker is no longer running".to_string(),
            })
    }

    pub(crate) fn try_recv(&self) -> Result<MqttWorkerEvent, mpsc::TryRecvError> {
        self.event_rx.try_recv()
    }

    pub(crate) fn stop(&mut self) {
        let _ = self.command_tx.send(MqttWorkerCommand::Stop);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl Drop for MqttTransportHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

enum MqttWorkerCommand {
    Publish(MqttPublishRequest),
    Stop,
}

fn worker_loop(
    config: MqttTransportConfig,
    command_rx: Receiver<MqttWorkerCommand>,
    event_tx: Sender<MqttWorkerEvent>,
    connected: Arc<AtomicBool>,
) {
    let mut mqtt_options = mqtt_options(&config);
    mqtt_options
        .set_max_packet_size(MQTT_MAX_PACKET_SIZE, MQTT_MAX_PACKET_SIZE)
        .set_clean_session(config.clean_session)
        .set_keep_alive(Duration::from_secs(config.keep_alive_secs));
    if let Some(credentials) = config.credentials.as_ref() {
        mqtt_options.set_credentials(credentials.username.clone(), credentials.password.clone());
    }

    let (client, mut connection) = Client::new(mqtt_options, MQTT_REQUEST_CHANNEL_CAPACITY);
    let mut network_options = connection.eventloop.network_options();
    network_options.set_connection_timeout(MQTT_CONNECT_TIMEOUT_SECS);
    connection.eventloop.set_network_options(network_options);

    let mut last_status: Option<MqttConnectionStatus> = None;

    loop {
        if drain_commands(&client, &command_rx, &event_tx) {
            break;
        }

        let poll_timeout = if connection.eventloop.network.is_none() {
            MQTT_CONNECT_POLL_TIMEOUT
        } else {
            MQTT_WORKER_POLL_INTERVAL
        };

        match connection.recv_timeout(poll_timeout) {
            Ok(Ok(event)) => {
                if !handle_mqtt_event(
                    &client,
                    &config,
                    &event_tx,
                    &connected,
                    &mut last_status,
                    event,
                ) {
                    break;
                }
            }
            Ok(Err(error)) => {
                if !enter_recovery(&event_tx, &connected, &mut last_status, error.to_string()) {
                    break;
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                let _ = event_tx.send(MqttWorkerEvent::Error(
                    "MQTT event loop disconnected from its request channel".to_string(),
                ));
                break;
            }
        }
    }

    connected.store(false, Ordering::Release);
    let _ = client.try_disconnect();
}

fn mqtt_options(config: &MqttTransportConfig) -> MqttOptions {
    MqttOptions::new(
        config.client_id.clone(),
        config.remote_host.clone(),
        config.remote_port,
    )
}

fn drain_commands(
    client: &Client,
    command_rx: &Receiver<MqttWorkerCommand>,
    event_tx: &Sender<MqttWorkerEvent>,
) -> bool {
    loop {
        match command_rx.try_recv() {
            Ok(MqttWorkerCommand::Publish(request)) => {
                if let Err(error) = client.try_publish(
                    request.topic.clone(),
                    request.qos.to_rumqttc(),
                    request.retain,
                    request.payload,
                ) {
                    let _ = event_tx.send(MqttWorkerEvent::Error(format!(
                        "failed to queue MQTT publish to '{}': {error}",
                        request.topic
                    )));
                }
            }
            Ok(MqttWorkerCommand::Stop) => return true,
            Err(mpsc::TryRecvError::Empty) => return false,
            Err(mpsc::TryRecvError::Disconnected) => return true,
        }
    }
}

fn handle_mqtt_event(
    client: &Client,
    config: &MqttTransportConfig,
    event_tx: &Sender<MqttWorkerEvent>,
    connected: &Arc<AtomicBool>,
    last_status: &mut Option<MqttConnectionStatus>,
    event: MqttEvent,
) -> bool {
    match event {
        MqttEvent::Incoming(Packet::ConnAck(_)) => {
            connected.store(true, Ordering::Release);
            if !emit_status(event_tx, last_status, MqttConnectionStatus::Connected) {
                return false;
            }
            subscribe_configured_filters(client, config, event_tx)
        }
        MqttEvent::Incoming(Packet::Publish(publish)) => event_tx
            .send(MqttWorkerEvent::Publish(MqttReceivedPublish {
                topic: publish.topic,
                payload: publish.payload.to_vec(),
                qos: mqtt_qos_from_rumqttc(publish.qos),
                retain: publish.retain,
            }))
            .is_ok(),
        MqttEvent::Incoming(Packet::Disconnect) => {
            enter_recovery(
                event_tx,
                connected,
                last_status,
                "MQTT broker disconnected".to_string(),
            )
        }
        MqttEvent::Incoming(_) | MqttEvent::Outgoing(_) => true,
    }
}

fn subscribe_configured_filters(
    client: &Client,
    config: &MqttTransportConfig,
    event_tx: &Sender<MqttWorkerEvent>,
) -> bool {
    for subscription in &config.subscriptions {
        if let Err(error) = client.try_subscribe(
            subscription.topic_filter.clone(),
            subscription.qos.to_rumqttc(),
        ) {
            if event_tx
                .send(MqttWorkerEvent::Error(format!(
                    "failed to subscribe MQTT filter '{}': {error}",
                    subscription.topic_filter
                )))
                .is_err()
            {
                return false;
            }
        }
    }

    true
}

fn enter_recovery(
    event_tx: &Sender<MqttWorkerEvent>,
    connected: &Arc<AtomicBool>,
    last_status: &mut Option<MqttConnectionStatus>,
    message: String,
) -> bool {
    connected.store(false, Ordering::Release);
    emit_status(event_tx, last_status, MqttConnectionStatus::Recovering { message })
}

fn emit_status(
    event_tx: &Sender<MqttWorkerEvent>,
    last_status: &mut Option<MqttConnectionStatus>,
    status: MqttConnectionStatus,
) -> bool {
    if last_status.as_ref() == Some(&status) {
        return true;
    }

    *last_status = Some(status.clone());
    event_tx.send(MqttWorkerEvent::Status(status)).is_ok()
}

fn mqtt_qos_from_rumqttc(qos: rumqttc::QoS) -> MqttQos {
    match qos {
        rumqttc::QoS::AtMostOnce => MqttQos::AtMost,
        rumqttc::QoS::AtLeastOnce => MqttQos::AtLeast,
        rumqttc::QoS::ExactlyOnce => MqttQos::Exactly,
    }
}

use std::{
    collections::BTreeSet,
    io,
    net::{IpAddr, SocketAddr, UdpSocket},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use artnet_protocol::{ArtCommand, Output};
use sacn::{
    packet::{
        universe_to_ipv4_multicast_addr, universe_to_ipv6_multicast_addr,
        AcnRootLayerProtocol, E131RootLayerData, ACN_SDT_MULTICAST_PORT,
    },
    source::SacnSource,
};

use super::frame::{DmxFrame, ARTNET_MAX_UNIVERSE, DMX_SLOT_COUNT};

const WORKER_COMMAND_CAPACITY: usize = 64;
const WORKER_IDLE_WAIT: Duration = Duration::from_millis(5);
const SACN_PACKET_CAPACITY: usize = 1_144;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DmxProtocol {
    ArtNet,
    Sacn,
}

impl DmxProtocol {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::ArtNet => "Art-Net",
            Self::Sacn => "sACN",
        }
    }

    pub(crate) const fn default_port(self) -> u16 {
        match self {
            Self::ArtNet => 6454,
            Self::Sacn => ACN_SDT_MULTICAST_PORT,
        }
    }

    pub(crate) const fn maximum_universe(self) -> u16 {
        match self {
            Self::ArtNet => ARTNET_MAX_UNIVERSE,
            Self::Sacn => super::frame::SACN_MAX_UNIVERSE,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DmxTransportConfig {
    pub protocol: DmxProtocol,
    pub bind_ip: IpAddr,
    pub listen_port: u16,
    pub receive_enabled: bool,
    pub universe: u16,
    pub destination: Option<SocketAddr>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum DmxWorkerEvent {
    Frame(DmxFrame),
    Error(String),
}

enum DmxWorkerCommand {
    Send(DmxFrame),
    Stop,
}

pub(crate) struct DmxTransportHandle {
    command_tx: SyncSender<DmxWorkerCommand>,
    latest_event: Arc<Mutex<Option<DmxWorkerEvent>>>,
    pending: Arc<AtomicBool>,
    replaced_frames: Arc<AtomicU64>,
    stop_requested: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl DmxTransportHandle {
    pub(crate) fn spawn(config: DmxTransportConfig) -> Result<Self, String> {
        validate_config(&config)?;
        let endpoint = WorkerEndpoint::open(&config)?;
        let (command_tx, command_rx) = mpsc::sync_channel(WORKER_COMMAND_CAPACITY);
        let latest_event = Arc::new(Mutex::new(None));
        let pending = Arc::new(AtomicBool::new(false));
        let replaced_frames = Arc::new(AtomicU64::new(0));
        let stop_requested = Arc::new(AtomicBool::new(false));
        let worker_event = Arc::clone(&latest_event);
        let worker_pending = Arc::clone(&pending);
        let worker_replaced_frames = Arc::clone(&replaced_frames);
        let worker_stop_requested = Arc::clone(&stop_requested);
        let thread_name = format!("chataigne-dmx-{}", config.protocol.label().to_ascii_lowercase());
        let thread = thread::Builder::new()
            .name(thread_name)
            .spawn(move || {
                worker_loop(
                    endpoint,
                    config,
                    command_rx,
                    worker_event,
                    worker_pending,
                    worker_replaced_frames,
                    worker_stop_requested,
                )
            })
            .map_err(|error| format!("failed to spawn DMX worker: {error}"))?;

        Ok(Self {
            command_tx,
            latest_event,
            pending,
            replaced_frames,
            stop_requested,
            thread: Some(thread),
        })
    }

    pub(crate) fn send(&self, frame: DmxFrame) -> Result<(), String> {
        self.command_tx
            .try_send(DmxWorkerCommand::Send(frame))
            .map_err(|error| match error {
                TrySendError::Full(_) => {
                    format!("DMX output queue is full ({WORKER_COMMAND_CAPACITY} frames)")
                }
                TrySendError::Disconnected(_) => "DMX worker is not running".to_string(),
            })?;
        if let Some(thread) = &self.thread {
            thread.thread().unpark();
        }
        Ok(())
    }

    pub(crate) fn has_pending(&self) -> bool {
        self.pending.load(Ordering::Acquire)
    }

    pub(crate) fn take_latest_event(&self) -> Option<DmxWorkerEvent> {
        let mut event = self
            .latest_event
            .lock()
            .expect("DMX latest-event mutex poisoned");
        let event = event.take();
        self.pending.store(false, Ordering::Release);
        event
    }

    pub(crate) fn take_replaced_frames(&self) -> u64 {
        self.replaced_frames.swap(0, Ordering::AcqRel)
    }

    pub(crate) fn stop(&mut self) {
        self.stop_requested.store(true, Ordering::Release);
        let _ = self.command_tx.try_send(DmxWorkerCommand::Stop);
        if let Some(thread) = self.thread.take() {
            thread.thread().unpark();
            let _ = thread.join();
        }
    }
}

impl Drop for DmxTransportHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

enum WorkerEndpoint {
    ArtNet { socket: UdpSocket },
    Sacn {
        source: SacnSource,
        receiver: Option<UdpSocket>,
        listen_universe: u16,
        registered_universes: BTreeSet<u16>,
    },
}

impl WorkerEndpoint {
    fn open(config: &DmxTransportConfig) -> Result<Self, String> {
        match config.protocol {
            DmxProtocol::ArtNet => {
                let bind_port = if config.receive_enabled {
                    config.listen_port
                } else {
                    0
                };
                let socket = UdpSocket::bind(SocketAddr::new(config.bind_ip, bind_port))
                    .map_err(|error| format!("failed to bind Art-Net UDP socket: {error}"))?;
                socket
                    .set_nonblocking(true)
                    .map_err(|error| format!("failed to configure Art-Net socket: {error}"))?;
                socket
                    .set_broadcast(true)
                    .map_err(|error| format!("failed to enable Art-Net broadcast: {error}"))?;
                Ok(Self::ArtNet { socket })
            }
            DmxProtocol::Sacn => {
                let mut source = SacnSource::with_ip(
                    "Chataigne2",
                    SocketAddr::new(config.bind_ip, 0),
                )
                .map_err(|error| format!("failed to create sACN source: {error}"))?;
                if config.destination.is_some() {
                    source.set_is_sending_discovery(false);
                }
                let receiver = open_sacn_receiver(config)?;
                Ok(Self::Sacn {
                    source,
                    receiver,
                    listen_universe: config.universe,
                    registered_universes: BTreeSet::new(),
                })
            }
        }
    }

    fn send(&mut self, frame: &DmxFrame, destination: Option<SocketAddr>) -> Result<(), String> {
        match self {
            Self::ArtNet { socket } => send_artnet(socket, frame, destination),
            Self::Sacn {
                source,
                registered_universes,
                ..
            } => send_sacn(source, registered_universes, frame, destination),
        }
    }

    fn receive(&mut self) -> Result<Option<DmxFrame>, String> {
        match self {
            Self::ArtNet { socket } => receive_artnet(socket),
            Self::Sacn {
                receiver,
                listen_universe,
                ..
            } => receive_sacn(receiver.as_ref(), *listen_universe),
        }
    }

    fn shutdown(&mut self) {
        if let Self::Sacn {
            source,
            registered_universes,
            ..
        } = self
        {
            for universe in std::mem::take(registered_universes) {
                let _ = source.terminate_stream(universe, 0);
            }
        }
    }
}

fn worker_loop(
    mut endpoint: WorkerEndpoint,
    config: DmxTransportConfig,
    command_rx: Receiver<DmxWorkerCommand>,
    latest_event: Arc<Mutex<Option<DmxWorkerEvent>>>,
    pending: Arc<AtomicBool>,
    replaced_frames: Arc<AtomicU64>,
    stop_requested: Arc<AtomicBool>,
) {
    let mut running = true;
    while running && !stop_requested.load(Ordering::Acquire) {
        loop {
            if stop_requested.load(Ordering::Acquire) {
                running = false;
                break;
            }
            match command_rx.try_recv() {
                Ok(DmxWorkerCommand::Send(frame)) => {
                    if let Err(error) = endpoint.send(&frame, config.destination) {
                        publish_event(
                            &latest_event,
                            &pending,
                            &replaced_frames,
                            DmxWorkerEvent::Error(error),
                        );
                    }
                }
                Ok(DmxWorkerCommand::Stop) | Err(TryRecvError::Disconnected) => {
                    running = false;
                    break;
                }
                Err(TryRecvError::Empty) => break,
            }
        }

        if !running {
            break;
        }

        match endpoint.receive() {
            Ok(Some(frame)) => {
                publish_event(
                    &latest_event,
                    &pending,
                    &replaced_frames,
                    DmxWorkerEvent::Frame(frame),
                );
            }
            Ok(None) => {}
            Err(error) => {
                publish_event(
                    &latest_event,
                    &pending,
                    &replaced_frames,
                    DmxWorkerEvent::Error(error),
                );
            }
        }

        thread::park_timeout(WORKER_IDLE_WAIT);
    }
    endpoint.shutdown();
}

fn publish_event(
    latest_event: &Mutex<Option<DmxWorkerEvent>>,
    pending: &AtomicBool,
    replaced_frames: &AtomicU64,
    event: DmxWorkerEvent,
) {
    let mut latest_event = latest_event
        .lock()
        .expect("DMX latest-event mutex poisoned");
    if matches!(
        (&*latest_event, &event),
        (Some(DmxWorkerEvent::Frame(_)), DmxWorkerEvent::Frame(_))
    ) {
        replaced_frames.fetch_add(1, Ordering::Relaxed);
    }
    *latest_event = Some(event);
    pending.store(true, Ordering::Release);
}

fn validate_config(config: &DmxTransportConfig) -> Result<(), String> {
    if config.universe == 0 || config.universe > config.protocol.maximum_universe() {
        return Err(format!(
            "{} universe must be between 1 and {}",
            config.protocol.label(),
            config.protocol.maximum_universe()
        ));
    }
    if config.receive_enabled && config.listen_port == 0 {
        return Err("DMX input port must be non-zero when input is enabled".to_string());
    }
    if config.protocol == DmxProtocol::ArtNet && config.destination.is_none() {
        return Err("Art-Net output requires a destination address".to_string());
    }
    Ok(())
}

fn open_sacn_receiver(config: &DmxTransportConfig) -> Result<Option<UdpSocket>, String> {
    if !config.receive_enabled {
        return Ok(None);
    }

    let socket = UdpSocket::bind(SocketAddr::new(config.bind_ip, config.listen_port))
        .map_err(|error| format!("failed to bind sACN UDP socket: {error}"))?;
    socket
        .set_nonblocking(true)
        .map_err(|error| format!("failed to configure sACN socket: {error}"))?;
    if config.listen_port == ACN_SDT_MULTICAST_PORT {
        join_sacn_multicast(&socket, config.bind_ip, config.universe)?;
    }
    Ok(Some(socket))
}

fn join_sacn_multicast(socket: &UdpSocket, bind_ip: IpAddr, universe: u16) -> Result<(), String> {
    match bind_ip {
        IpAddr::V4(interface) => {
            let multicast = universe_to_ipv4_multicast_addr(universe)
                .map_err(|error| format!("failed to resolve sACN multicast address: {error}"))?
                .as_socket()
                .ok_or_else(|| "sACN IPv4 multicast address was not an IP socket".to_string())?;
            let SocketAddr::V4(multicast) = multicast else {
                return Err("sACN IPv4 multicast address used the wrong IP family".to_string());
            };
            socket
                .join_multicast_v4(multicast.ip(), &interface)
                .map_err(|error| format!("failed to join sACN multicast universe: {error}"))
        }
        IpAddr::V6(_) => {
            let multicast = universe_to_ipv6_multicast_addr(universe)
                .map_err(|error| format!("failed to resolve sACN multicast address: {error}"))?
                .as_socket()
                .ok_or_else(|| "sACN IPv6 multicast address was not an IP socket".to_string())?;
            let SocketAddr::V6(multicast) = multicast else {
                return Err("sACN IPv6 multicast address used the wrong IP family".to_string());
            };
            socket
                .join_multicast_v6(multicast.ip(), 0)
                .map_err(|error| format!("failed to join sACN multicast universe: {error}"))
        }
    }
}

fn send_artnet(
    socket: &UdpSocket,
    frame: &DmxFrame,
    destination: Option<SocketAddr>,
) -> Result<(), String> {
    let destination = destination.ok_or_else(|| "Art-Net destination is missing".to_string())?;
    let port_address = (frame.universe - 1)
        .try_into()
        .map_err(|error| format!("invalid Art-Net universe: {error}"))?;
    let output = Output {
        sequence: frame.sequence,
        port_address,
        data: frame.slots.clone().into(),
        ..Output::default()
    };
    let packet = ArtCommand::Output(output)
        .write_to_buffer()
        .map_err(|error| format!("failed to encode ArtDMX frame: {error}"))?;
    socket
        .send_to(packet.as_slice(), destination)
        .map_err(|error| format!("failed to send ArtDMX frame: {error}"))?;
    Ok(())
}

fn receive_artnet(socket: &UdpSocket) -> Result<Option<DmxFrame>, String> {
    let mut bytes = [0_u8; 1_024];
    let length = match socket.recv_from(&mut bytes) {
        Ok((length, _)) => length,
        Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(None),
        Err(error) => return Err(format!("failed to receive Art-Net packet: {error}")),
    };
    let command = ArtCommand::from_buffer(&bytes[..length])
        .map_err(|error| format!("failed to decode Art-Net packet: {error}"))?;
    let ArtCommand::Output(output) = command else {
        return Ok(None);
    };
    let universe = u16::from(output.port_address).saturating_add(1);
    let slots = output
        .data
        .as_ref()
        .iter()
        .copied()
        .take(DMX_SLOT_COUNT)
        .collect();
    DmxFrame::with_metadata(universe, output.sequence, 100, slots).map(Some)
}

fn send_sacn(
    source: &mut SacnSource,
    registered_universes: &mut BTreeSet<u16>,
    frame: &DmxFrame,
    destination: Option<SocketAddr>,
) -> Result<(), String> {
    if registered_universes.insert(frame.universe) {
        source
            .register_universe(frame.universe)
            .map_err(|error| format!("failed to register sACN universe: {error}"))?;
    }
    let mut payload = Vec::with_capacity(frame.slots.len() + 1);
    payload.push(0);
    payload.extend_from_slice(frame.slots.as_slice());
    source
        .send(
            &[frame.universe],
            payload.as_slice(),
            Some(frame.priority),
            destination,
            None,
        )
        .map_err(|error| format!("failed to send sACN frame: {error}"))
}

fn receive_sacn(
    receiver: Option<&UdpSocket>,
    listen_universe: u16,
) -> Result<Option<DmxFrame>, String> {
    let Some(receiver) = receiver else {
        return Ok(None);
    };
    let mut bytes = [0_u8; SACN_PACKET_CAPACITY];
    let length = match receiver.recv_from(&mut bytes) {
        Ok((length, _)) => length,
        Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(None),
        Err(error) => return Err(format!("failed to receive sACN packet: {error}")),
    };
    let packet = AcnRootLayerProtocol::parse(&bytes[..length])
        .map_err(|error| format!("failed to decode sACN packet: {error}"))?;
    let E131RootLayerData::DataPacket(frame) = packet.pdu.data else {
        return Ok(None);
    };
    if frame.universe != listen_universe
        || frame.preview_data
        || frame.stream_terminated
        || frame.synchronization_address != 0
    {
        return Ok(None);
    }
    let mut slots = frame.data.property_values.into_owned();
    if slots.first() == Some(&0) {
        slots.remove(0);
    }
    slots.truncate(DMX_SLOT_COUNT);
    DmxFrame::with_metadata(
        frame.universe,
        frame.sequence_number,
        frame.priority,
        slots,
    )
    .map(Some)
}

#[cfg(test)]
mod tests;

use std::{
    io::{Read, Write},
    sync::mpsc::{self, Receiver, Sender},
    thread::{self, JoinHandle},
    time::Duration,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SerialStreamingTransportConfig {
    pub port_name: String,
    pub baud_rate: u32,
    pub receive_enabled: bool,
    pub send_enabled: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum StreamingWorkerEvent {
    Bytes(Vec<u8>),
    Error(String),
    Stopped(String),
}

pub(crate) struct SerialStreamingTransportHandle {
    command_tx: Sender<SerialStreamingWorkerCommand>,
    event_rx: Receiver<StreamingWorkerEvent>,
    worker: Option<JoinHandle<()>>,
}

impl SerialStreamingTransportHandle {
    pub(crate) fn spawn(config: SerialStreamingTransportConfig) -> Result<Self, String> {
        if config.port_name.trim().is_empty() {
            return Err("serial port name cannot be empty".to_string());
        }

        let port = serialport::new(config.port_name.as_str(), config.baud_rate)
            .timeout(Duration::from_millis(5))
            .open()
            .map_err(|error| format!("failed to open serial port '{}': {error}", config.port_name))?;

        let (command_tx, command_rx) = mpsc::channel();
        let (event_tx, event_rx) = mpsc::channel();

        let worker = thread::Builder::new()
            .name(format!("streaming-serial-{}", config.port_name))
            .spawn(move || worker_loop(port, config, command_rx, event_tx))
            .map_err(|error| format!("failed to start serial worker thread: {error}"))?;

        Ok(Self {
            command_tx,
            event_rx,
            worker: Some(worker),
        })
    }

    pub(crate) fn send(&self, bytes: Vec<u8>) -> Result<(), String> {
        self.command_tx
            .send(SerialStreamingWorkerCommand::Send(bytes))
            .map_err(|_| "serial worker is no longer running".to_string())
    }

    pub(crate) fn try_recv(&self) -> Result<StreamingWorkerEvent, mpsc::TryRecvError> {
        self.event_rx.try_recv()
    }

    pub(crate) fn stop(&mut self) {
        let _ = self.command_tx.send(SerialStreamingWorkerCommand::Stop);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl Drop for SerialStreamingTransportHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

enum SerialStreamingWorkerCommand {
    Send(Vec<u8>),
    Stop,
}

fn worker_loop(
    mut port: Box<dyn serialport::SerialPort>,
    config: SerialStreamingTransportConfig,
    command_rx: Receiver<SerialStreamingWorkerCommand>,
    event_tx: Sender<StreamingWorkerEvent>,
) {
    let mut buffer = [0u8; 8192];

    loop {
        if drain_commands(&command_rx, &event_tx, &mut port, config.send_enabled) {
            break;
        }

        if config.receive_enabled {
            match port.read(&mut buffer) {
                Ok(length) if length > 0 => {
                    if event_tx
                        .send(StreamingWorkerEvent::Bytes(buffer[..length].to_vec()))
                        .is_err()
                    {
                        return;
                    }
                }
                Ok(_) => {}
                Err(error)
                    if error.kind() == std::io::ErrorKind::TimedOut
                        || error.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(error) => {
                    let _ = event_tx.send(StreamingWorkerEvent::Stopped(format!(
                        "serial port receive error: {error}"
                    )));
                    break;
                }
            }
        }

        match command_rx.recv_timeout(Duration::from_millis(5)) {
            Ok(SerialStreamingWorkerCommand::Send(bytes)) => {
                write_bytes(&event_tx, &mut port, config.send_enabled, bytes);
            }
            Ok(SerialStreamingWorkerCommand::Stop) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
}

fn drain_commands(
    command_rx: &Receiver<SerialStreamingWorkerCommand>,
    event_tx: &Sender<StreamingWorkerEvent>,
    port: &mut Box<dyn serialport::SerialPort>,
    send_enabled: bool,
) -> bool {
    loop {
        match command_rx.try_recv() {
            Ok(SerialStreamingWorkerCommand::Send(bytes)) => write_bytes(event_tx, port, send_enabled, bytes),
            Ok(SerialStreamingWorkerCommand::Stop) => return true,
            Err(mpsc::TryRecvError::Empty) => return false,
            Err(mpsc::TryRecvError::Disconnected) => return true,
        }
    }
}

fn write_bytes(
    event_tx: &Sender<StreamingWorkerEvent>,
    port: &mut Box<dyn serialport::SerialPort>,
    send_enabled: bool,
    bytes: Vec<u8>,
) {
    if !send_enabled {
        let _ = event_tx.send(StreamingWorkerEvent::Error(
            "serial sender is disabled; outgoing bytes were dropped".to_string(),
        ));
        return;
    }

    if let Err(error) = port.write_all(bytes.as_slice()) {
        let _ = event_tx.send(StreamingWorkerEvent::Stopped(format!(
            "failed to write serial port: {error}"
        )));
    }
}

//! The shared host publishes a coherent OSCQuery hierarchy and accepts OSC on an opt-in port.
mod commands;
mod graph;
mod input;
mod lan;
mod preferences;
mod values;
mod websocket;

use super::{
    ConnectionLimiter, HttpResponse, RequestAccessPolicy, ServerState, ServerWorkers, is_websocket_upgrade_request,
    write_json, write_json_error, write_response,
};
use golden_engine::{app::ProjectLifecycle, ui_read_model::UiReadModel};
use golden_protocol::UiSubscriptionScope;
use preferences::Settings;
use std::{
    io,
    net::{Ipv4Addr, SocketAddrV4, TcpStream},
    sync::{Arc, mpsc::SyncSender},
    time::{Duration, Instant},
};
use tokio::net::{TcpListener, UdpSocket};

pub(crate) use preferences::preferences;

const SETTINGS_INTERVAL: Duration = Duration::from_millis(200);
const RETRY_INTERVAL: Duration = Duration::from_secs(1);
const IO_TIMEOUT: Duration = Duration::from_secs(1);
const MAX_CONNECTIONS: usize = 4 + websocket::MAX_CLIENTS;

pub(super) struct Listener {
    sockets: Option<(TcpListener, UdpSocket)>,
    settings: Option<Settings>,
    refresh_at: Instant,
    retry_at: Instant,
    bind_failed: bool,
    limiter: Arc<ConnectionLimiter>,
    websocket_limiter: Arc<ConnectionLimiter>,
    access: Arc<RequestAccessPolicy>,
    input: SyncSender<input::Datagram>,
    datagram: Vec<u8>,
    lan: lan::Lan,
}

impl Listener {
    pub(super) fn new<T: ProjectLifecycle + 'static>(
        state: &ServerState<T>,
        workers: &mut ServerWorkers,
    ) -> io::Result<Self> {
        let now = Instant::now();
        Ok(Self {
            sockets: None,
            settings: None,
            refresh_at: now,
            retry_at: now,
            bind_failed: false,
            limiter: Arc::new(ConnectionLimiter::new(MAX_CONNECTIONS)),
            websocket_limiter: Arc::new(ConnectionLimiter::new(websocket::MAX_CLIENTS)),
            access: Arc::new(RequestAccessPolicy::new(&[], &[])?),
            input: input::spawn(state, workers)?,
            // Read the entire datagram so oversized UDP packets are discarded consistently across platforms.
            datagram: vec![0; usize::from(u16::MAX)],
            lan: lan::Lan::new(T::application_display_name())?,
        })
    }

    pub(super) async fn next(&mut self, model: &UiReadModel) -> Option<tokio::net::TcpStream> {
        let now = Instant::now();
        if now >= self.refresh_at {
            self.refresh_at = now + SETTINGS_INTERVAL;
            let snapshot = model.capture_snapshot(UiSubscriptionScope::WholeGraph);
            let settings = preferences::enabled_settings(&snapshot);
            if settings != self.settings {
                self.settings = settings;
                self.sockets = None;
                self.lan.stop(now);
                self.retry_at = now;
                self.bind_failed = false;
            }
            if let Some(settings) = self.settings {
                if self.sockets.is_none() && now >= self.retry_at {
                    match self.bind(settings).await {
                        Ok(()) => self.bind_failed = false,
                        Err(error) => {
                            if !self.bind_failed {
                                eprintln!(
                                    "[oscquery] could not listen on port {}: {error}; retrying",
                                    settings.port
                                );
                            }
                            self.bind_failed = true;
                            self.retry_at = now + RETRY_INTERVAL;
                        }
                    }
                }
                if settings.allow_lan && self.sockets.is_some() {
                    self.lan.refresh(settings.port, now);
                }
            }
        }
        let delay = tokio::time::sleep_until(self.refresh_at.into());
        let (Some((tcp, udp)), Some(settings)) = (&self.sockets, self.settings) else {
            delay.await;
            return None;
        };
        let accepted = tokio::select! {
            accepted = tcp.accept() => accepted.map(|(stream, peer)| self.lan.permits(peer.ip()).then_some(stream)),
            received = udp.recv_from(&mut self.datagram) => {
                match received {
                    Ok((size, peer)) => {
                        if size <= input::MAX_DATAGRAM && self.lan.permits(peer.ip()) {
                            let _ = self.input.try_send(input::Datagram {
                                settings,
                                project: model.current_project_generation(),
                                bytes: self.datagram[..size].to_vec(),
                            });
                        }
                        return None;
                    }
                    Err(error) => Err(error),
                }
            }
            () = delay => return None,
        };
        match accepted {
            Ok(stream) => stream,
            Err(error) => {
                eprintln!("[oscquery] listener failed: {error}; retrying");
                self.sockets = None;
                let now = Instant::now();
                self.lan.stop(now);
                self.retry_at = now + RETRY_INTERVAL;
                None
            }
        }
    }

    async fn bind(&mut self, settings: Settings) -> io::Result<()> {
        let mut allowed_hosts = Vec::new();
        let ip = if settings.allow_lan {
            let hostname = self.lan.hostname();
            allowed_hosts.push(format!("{hostname}:{}", settings.port));
            allowed_hosts.push(format!("{}:{}", hostname.trim_end_matches('.'), settings.port));
            Ipv4Addr::UNSPECIFIED
        } else {
            Ipv4Addr::LOCALHOST
        };
        let access = RequestAccessPolicy::new(&[], &allowed_hosts)?;
        let address = SocketAddrV4::new(ip, settings.port);
        let udp = UdpSocket::bind(address).await?;
        let tcp = TcpListener::bind(address).await?;
        self.access = Arc::new(access);
        self.sockets = Some((tcp, udp));
        Ok(())
    }

    pub(super) fn accept<T: ProjectLifecycle>(
        &self,
        stream: tokio::net::TcpStream,
        state: &ServerState<T>,
        workers: &mut ServerWorkers,
    ) -> io::Result<()> {
        let Some(settings) = self.settings else {
            return Ok(());
        };
        let Some(permit) = self.limiter.try_acquire() else {
            return Ok(());
        };
        let mut stream = stream.into_std()?;
        stream.set_nonblocking(false)?;
        let connections = workers.connections.clone();
        let access = self.access.clone();
        let websocket_limiter = self.websocket_limiter.clone();
        let state = state.clone();
        workers.spawn("golden-oscquery-connection", move || {
            let _permit = permit;
            let Ok(_connection) = connections.track(&stream) else {
                return;
            };
            let _ = respond(&mut stream, &state, &access, &websocket_limiter, settings);
        })
    }
}

fn respond<T: ProjectLifecycle>(
    stream: &mut TcpStream,
    state: &ServerState<T>,
    access: &RequestAccessPolicy,
    websocket_limiter: &Arc<ConnectionLimiter>,
    settings: Settings,
) -> io::Result<()> {
    stream.set_read_timeout(Some(IO_TIMEOUT))?;
    stream.set_write_timeout(Some(IO_TIMEOUT))?;
    let request = match super::read_http_request(stream) {
        Ok(request) => request,
        Err(_) => {
            return write_json_error(
                &mut HttpResponse::private(stream),
                "400 Bad Request",
                "Invalid OSCQuery request",
            );
        }
    };
    let address = stream.local_addr()?;
    let mut response = HttpResponse::private(stream);
    if let Err(error) = access.admit(&request, address) {
        return write_json_error(&mut response, "403 Forbidden", error);
    }
    if request.method != "GET" {
        return write_json_error(&mut response, "405 Method Not Allowed", "OSCQuery accepts GET");
    }
    if !request.body.is_empty() {
        return write_json_error(&mut response, "400 Bad Request", "Unexpected OSCQuery request body");
    }
    let snapshot = state.read_model.capture_snapshot(UiSubscriptionScope::WholeGraph);
    if preferences::enabled_settings(&snapshot) != Some(settings) {
        return write_json_error(&mut response, "503 Service Unavailable", "OSCQuery listener changed");
    }
    if request.path == "/" && is_websocket_upgrade_request(&request) {
        let Some(_permit) = websocket_limiter.try_acquire() else {
            return write_json_error(
                &mut response,
                "503 Service Unavailable",
                "OSCQuery WebSocket limit reached",
            );
        };
        drop(snapshot);
        return websocket::serve(stream, &request, state, settings);
    }
    if request.query.as_deref() == Some("HOST_INFO") {
        return write_json(
            &mut response,
            "200 OK",
            &serde_json::json!({
                "NAME": T::application_display_name(),
                "OSC_IP": address.ip().to_string(),
                "OSC_PORT": address.port(),
                "OSC_TRANSPORT": "UDP",
                "WS_PORT": address.port(),
                "EXTENSIONS": {
                    "ACCESS": true, "TYPE": true, "VALUE": true, "RANGE": true,
                    "GOLDEN_META": true, "GOLDEN_ADD_TYPES": true, "GOLDEN_COMMANDS": true
                }
            }),
        );
    }
    match graph::describe(&snapshot, &request.path, request.query.as_deref()) {
        Ok(Some(bytes)) => write_response(&mut response, "200 OK", "application/json", &bytes),
        Ok(None) => write_response(&mut response, "204 No Content", "application/json", &[]),
        Err(error) => write_json_error(&mut response, error.status(), error.message()),
    }
}

#[cfg(test)]
mod tests;

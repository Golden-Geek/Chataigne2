//! The shared host publishes a coherent, read-only OSCQuery hierarchy on an opt-in port.
mod graph;
mod preferences;
mod values;

use super::{
    ConnectionLimiter, HttpResponse, RequestAccessPolicy, ServerWorkers, write_json, write_json_error, write_response,
};
use golden_engine::{app::ProjectLifecycle, ui_read_model::UiReadModel};
use std::{
    io,
    net::{Ipv4Addr, SocketAddrV4, TcpStream},
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::net::TcpListener;

pub(crate) use preferences::preferences;

const SETTINGS_INTERVAL: Duration = Duration::from_millis(200);
const RETRY_INTERVAL: Duration = Duration::from_secs(1);
const IO_TIMEOUT: Duration = Duration::from_secs(1);
const MAX_CONNECTIONS: usize = 4;

pub(super) struct Listener {
    listener: Option<TcpListener>,
    port: Option<u16>,
    refresh_at: Instant,
    retry_at: Instant,
    bind_failed: bool,
    limiter: Arc<ConnectionLimiter>,
    access: Arc<RequestAccessPolicy>,
}

impl Listener {
    pub(super) fn new() -> io::Result<Self> {
        let now = Instant::now();
        Ok(Self {
            listener: None,
            port: None,
            refresh_at: now,
            retry_at: now,
            bind_failed: false,
            limiter: Arc::new(ConnectionLimiter::new(MAX_CONNECTIONS)),
            access: Arc::new(RequestAccessPolicy::new(&[], &[])?),
        })
    }

    pub(super) async fn next(&mut self, model: &UiReadModel) -> Option<tokio::net::TcpStream> {
        let now = Instant::now();
        if now >= self.refresh_at {
            self.refresh_at = now + SETTINGS_INTERVAL;
            let port = preferences::enabled_port(model);
            if port != self.port {
                self.port = port;
                self.listener = None;
                self.retry_at = now;
                self.bind_failed = false;
            }
            if let Some(port) = self.port
                && self.listener.is_none()
                && now >= self.retry_at
            {
                match TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port)).await {
                    Ok(listener) => {
                        self.listener = Some(listener);
                        self.bind_failed = false;
                    }
                    Err(error) => {
                        if !self.bind_failed {
                            eprintln!("[oscquery] could not listen on port {port}: {error}; retrying");
                        }
                        self.bind_failed = true;
                        self.retry_at = now + RETRY_INTERVAL;
                    }
                }
            }
        }
        let delay = tokio::time::sleep_until(self.refresh_at.into());
        let Some(listener) = &self.listener else {
            delay.await;
            return None;
        };
        let accepted = match futures_util::future::select(Box::pin(listener.accept()), Box::pin(delay)).await {
            futures_util::future::Either::Left((result, _)) => result,
            futures_util::future::Either::Right(_) => return None,
        };
        match accepted {
            Ok((stream, _)) => Some(stream),
            Err(error) => {
                eprintln!("[oscquery] listener failed: {error}; retrying");
                self.listener = None;
                self.retry_at = Instant::now() + RETRY_INTERVAL;
                None
            }
        }
    }

    pub(super) fn accept<T: ProjectLifecycle>(
        &self,
        stream: tokio::net::TcpStream,
        model: Arc<UiReadModel>,
        workers: &mut ServerWorkers,
    ) -> io::Result<()> {
        let Some(permit) = self.limiter.try_acquire() else {
            return Ok(());
        };
        let mut stream = stream.into_std()?;
        stream.set_nonblocking(false)?;
        let connections = workers.connections.clone();
        let access = self.access.clone();
        workers.spawn("golden-oscquery-connection", move || {
            let _permit = permit;
            let Ok(_connection) = connections.track(&stream) else {
                return;
            };
            let _ = respond(&mut stream, &model, &access, T::application_display_name());
        })
    }
}

fn respond(stream: &mut TcpStream, model: &UiReadModel, access: &RequestAccessPolicy, name: &str) -> io::Result<()> {
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
    if request.query.as_deref() == Some("HOST_INFO") {
        return write_json(
            &mut response,
            "200 OK",
            &serde_json::json!({
                "NAME": name,
                "EXTENSIONS": { "ACCESS": true, "TYPE": true, "VALUE": true, "RANGE": true }
            }),
        );
    }
    let snapshot = model.capture_snapshot(golden_protocol::UiSubscriptionScope::WholeGraph);
    match graph::describe(&snapshot, &request.path, request.query.as_deref()) {
        Ok(Some(bytes)) => write_response(&mut response, "200 OK", "application/json", &bytes),
        Ok(None) => write_response(&mut response, "204 No Content", "application/json", &[]),
        Err(error) => write_json_error(&mut response, error.status(), error.message()),
    }
}

#[cfg(test)]
mod tests;

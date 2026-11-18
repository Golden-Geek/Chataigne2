use super::{Settings, commands, feedback::Feedback, preferences};
use crate::ui_server::{
    HttpRequest, ServerState, build_websocket_request, create_response, write_ws_handshake_response,
};
use futures_util::{SinkExt, StreamExt};
use golden_engine::app::ProjectLifecycle;
use golden_io::WorkerNotifier;
use golden_protocol::UiSubscriptionScope;
use std::{
    io::{self, Write},
    net::TcpStream,
    sync::Arc,
    time::Duration,
};
use tokio::sync::Notify;
use tokio_tungstenite::{
    WebSocketStream,
    tungstenite::{
        self, Message,
        protocol::{Role, WebSocketConfig},
    },
};

pub(super) const MAX_CLIENTS: usize = 8;
const MAX_MESSAGE: usize = 16 * 1024;
const IDLE_CHECK: Duration = Duration::from_millis(150);
const SEND_TIMEOUT: Duration = Duration::from_secs(1);

type Socket = WebSocketStream<tokio::net::TcpStream>;

pub(super) fn serve<T: ProjectLifecycle>(
    stream: &mut TcpStream,
    request: &HttpRequest,
    state: &ServerState<T>,
    settings: Settings,
) -> io::Result<()> {
    let request = build_websocket_request(request)?;
    match create_response(&request) {
        Ok(response) => write_ws_handshake_response(&mut *stream, &response).map_err(io::Error::other)?,
        Err(tungstenite::Error::Http(response)) => {
            write_ws_handshake_response(&mut *stream, &response).map_err(io::Error::other)?;
            return stream.flush();
        }
        Err(error) => return Err(io::Error::other(error)),
    }
    stream.flush()?;
    stream.set_nodelay(true)?;
    stream.set_read_timeout(None)?;
    stream.set_write_timeout(None)?;
    stream.set_nonblocking(true)?;
    let config = WebSocketConfig::default()
        .read_buffer_size(MAX_MESSAGE)
        .write_buffer_size(0)
        .max_message_size(Some(MAX_MESSAGE))
        .max_frame_size(Some(MAX_MESSAGE));
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
    let stream = stream.try_clone()?;
    runtime.block_on(async {
        let stream = tokio::net::TcpStream::from_std(stream)?;
        let socket = WebSocketStream::from_raw_socket(stream, Role::Server, Some(config)).await;
        run(socket, state, settings).await
    })
}

async fn run<T: ProjectLifecycle>(mut socket: Socket, state: &ServerState<T>, settings: Settings) -> io::Result<()> {
    let model = &state.read_model;
    let revisions = model.event_revision();
    let wake = Arc::new(Notify::new());
    let notifier = {
        let wake = wake.clone();
        WorkerNotifier::new(move || wake.notify_one())
    };
    let observed = revisions.current();
    let mut feedback = Feedback::new(model.current_cursor(), observed);
    let mut idle = tokio::time::interval(IDLE_CHECK);
    idle.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            _ = idle.tick() => {
                if !current(state, settings, &feedback) {
                    break;
                }
            }
            buffered = feedback.wait(&revisions, &notifier, &wake) => {
                if !current(state, settings, &feedback) {
                    break;
                }
                match feedback.collect(model, revisions.current(), buffered) {
                    Ok(Some(frame)) => send(&mut socket, Message::binary(frame)).await?,
                    Ok(None) => {}
                    Err(_) => break,
                }
            }
            incoming = socket.next() => match incoming {
                Some(Ok(Message::Text(text))) => {
                    if !current(state, settings, &feedback) {
                        break;
                    }
                    let reply = commands::handle(state, settings, &mut feedback, text.as_str());
                    send(&mut socket, Message::text(reply.data.to_string())).await?;
                    if let Some(frame) = reply.initial_value {
                        send(&mut socket, Message::binary(frame)).await?;
                    }
                }
                Some(Ok(Message::Ping(_))) => {
                    tokio::time::timeout(SEND_TIMEOUT, socket.flush()).await?.map_err(io::Error::other)?;
                }
                Some(Ok(Message::Close(_))) => {
                    let _ = tokio::time::timeout(SEND_TIMEOUT, socket.flush()).await;
                    return Ok(());
                }
                Some(Ok(_)) => {}
                None | Some(Err(tungstenite::Error::ConnectionClosed | tungstenite::Error::AlreadyClosed)) => return Ok(()),
                Some(Err(error)) => return Err(io::Error::other(error)),
            }
        }
    }
    let _ = tokio::time::timeout(SEND_TIMEOUT, socket.close(None)).await;
    Ok(())
}

fn current<T: ProjectLifecycle>(state: &ServerState<T>, settings: Settings, feedback: &Feedback) -> bool {
    let snapshot = state.read_model.capture_snapshot(UiSubscriptionScope::WholeGraph);
    snapshot.project_generation().get() == feedback.project_generation()
        && preferences::enabled_settings(&snapshot) == Some(settings)
}

async fn send(socket: &mut Socket, message: Message) -> io::Result<()> {
    tokio::time::timeout(SEND_TIMEOUT, socket.send(message))
        .await?
        .map_err(io::Error::other)
}

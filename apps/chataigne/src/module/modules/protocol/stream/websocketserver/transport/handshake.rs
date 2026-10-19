use super::Event;
use crate::app::module::common::streaming::websocket::{
    socket::{BudgetSocket, WireSocket},
    websocket_config,
};
use std::{
    cell::RefCell,
    net::{SocketAddr, TcpStream},
    rc::Rc,
    time::{Duration, Instant},
};
use tokio_tungstenite::tungstenite::{
    accept_hdr_with_config,
    handshake::{
        server::{Callback, ErrorResponse, Request, Response, ServerHandshake},
        HandshakeError, MidHandshake,
    },
    http::{Response as HttpResponse, StatusCode},
};

struct PathCallback {
    expected: String,
    actual: Rc<RefCell<String>>,
}
impl Callback for PathCallback {
    fn on_request(self, request: &Request, response: Response) -> Result<Response, ErrorResponse> {
        let path = request.uri().path();
        let rejection = if path.len() > 8192 {
            Some((StatusCode::URI_TOO_LONG, "WebSocket request path exceeds 8192 bytes"))
        } else if !self.expected.is_empty() && path != self.expected {
            Some((StatusCode::NOT_FOUND, "websocket path not found"))
        } else {
            None
        };
        if let Some((status, message)) = rejection {
            return Err(HttpResponse::builder()
                .status(status)
                .body(Some(message.into()))
                .expect("valid WebSocket rejection"));
        }
        *self.actual.borrow_mut() = path.to_owned();
        Ok(response)
    }
}
type Handshake = MidHandshake<ServerHandshake<BudgetSocket, PathCallback>>;
pub(super) struct Pending {
    address: SocketAddr,
    actual: Rc<RefCell<String>>,
    state: Option<Handshake>,
    initial: Option<TcpStream>,
    expected: String,
    started: Instant,
}
pub(super) enum Outcome {
    Pending,
    Connected(Box<WireSocket>, String, String),
    Failed(Event),
}
impl Pending {
    pub fn start(stream: TcpStream, address: SocketAddr, path: &str) -> Result<Self, Event> {
        let actual = Rc::new(RefCell::new(String::new()));
        let setup = stream.set_nonblocking(true).and_then(|()| stream.set_nodelay(true));
        setup.map_err(|error| Event::Error(format!("WebSocket peer setup failed: {error}")))?;
        Ok(Self {
            address,
            actual,
            state: None,
            initial: Some(stream),
            expected: path.into(),
            started: Instant::now(),
        })
    }
    fn advance(
        &mut self,
        result: Result<WireSocket, HandshakeError<ServerHandshake<BudgetSocket, PathCallback>>>,
    ) -> Outcome {
        match result {
            Ok(socket) => Outcome::Connected(
                Box::new(socket),
                self.address.to_string(),
                format!("Remote address: {}, Path: {}", self.address, self.actual.borrow()),
            ),
            Err(HandshakeError::Interrupted(state)) => {
                self.state = Some(state);
                Outcome::Pending
            }
            Err(HandshakeError::Failure(error)) => Outcome::Failed(Event::Error(format!(
                "WebSocket handshake failed for {}: {error}",
                self.address
            ))),
        }
    }
    pub fn turn(&mut self) -> Outcome {
        if self.started.elapsed() >= Duration::from_secs(3) {
            return Outcome::Failed(Event::Error(format!(
                "WebSocket handshake deadline exceeded for {}",
                self.address
            )));
        }
        if let Some(stream) = self.initial.take() {
            let result = accept_hdr_with_config(
                BudgetSocket::new(stream),
                PathCallback {
                    expected: std::mem::take(&mut self.expected),
                    actual: self.actual.clone(),
                },
                Some(websocket_config()),
            );
            return self.advance(result);
        }
        let mut state = self.state.take().expect("pending WebSocket handshake");
        state.get_mut().get_mut().reset();
        self.advance(state.handshake())
    }
}

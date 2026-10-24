use super::*;
use golden_core::ui_sync::{
    UI_PROTOCOL_VERSION, UiClientMessage, UiControlPhase, UiProjectPathDto, UiProjectUploadRequest, UiServerMessage,
    UiSessionRequest, UiSessionResponse, UiSnapshotRequest, UiSubscriptionScope,
};
use reqwest::{
    blocking::Client,
    header::{AUTHORIZATION, HeaderMap, HeaderValue},
};
use serde::{Serialize, de::DeserializeOwned};
use std::{
    fs,
    net::{TcpListener, TcpStream},
    path::PathBuf,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};
use tokio_tungstenite::tungstenite::{self, Message, client::IntoClientRequest, stream::MaybeTlsStream};

pub(super) struct Host {
    child: Child,
    pub(super) base: String,
    pub(super) client: Client,
    websocket_protocols: Vec<String>,
    pub(super) evidence: PathBuf,
}

impl Host {
    pub(super) fn start() -> Self {
        let binary = std::env::current_exe()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join(format!("Chataigne2{}", std::env::consts::EXE_SUFFIX));
        assert!(
            binary.is_file(),
            "build the current default Chataigne2 binary before this acceptance case"
        );
        let evidence = std::env::temp_dir().join(format!("chataigne-p08-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&evidence).unwrap();
        let log = fs::File::create(evidence.join("headless.log")).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        drop(listener);
        let mut command = Command::new(&binary);
        command
            .args(["--headless", "--no-frontend", "--no-remote"])
            .env("GC_UI_BIND", address.to_string())
            .env_remove("GC_UI_FRONTEND_URL")
            .env_remove("CHATAIGNE_BUILTIN_FORMULAS_DIR")
            .env("APPDATA", &evidence)
            .env("XDG_DATA_HOME", &evidence)
            .env("HOME", &evidence)
            .env("USERPROFILE", &evidence)
            .stdout(Stdio::from(log.try_clone().unwrap()))
            .stderr(Stdio::from(log));
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            command.creation_flags(CREATE_NO_WINDOW);
        }
        let child = command.spawn().unwrap();
        let client = Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap();
        let mut host = Self {
            child,
            base: format!("http://{address}"),
            client,
            websocket_protocols: Vec::new(),
            evidence,
        };
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            if let Some(status) = host.child.try_wait().unwrap() {
                panic!("headless child exited {status}; evidence {}", host.evidence.display());
            }
            if host
                .client
                .get(format!("{}/api/ui/health", host.base))
                .send()
                .is_ok_and(|reply| reply.status().is_success())
            {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "headless readiness deadline; evidence {}",
                host.evidence.display()
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        let session: UiSessionResponse = host.post("/api/ui/session", &UiSessionRequest::default());
        assert_eq!(session.protocol_version, UI_PROTOCOL_VERSION);
        let mut authorization = HeaderValue::from_str(&format!("Bearer {}", session.token)).unwrap();
        authorization.set_sensitive(true);
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, authorization);
        host.client = Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(10))
            .default_headers(headers)
            .build()
            .unwrap();
        host.websocket_protocols = session.websocket_protocols;
        host
    }

    pub(super) fn post<S: Serialize, R: DeserializeOwned>(&self, path: &str, payload: &S) -> R {
        self.client
            .post(format!("{}{path}", self.base))
            .json(payload)
            .send()
            .unwrap()
            .error_for_status()
            .unwrap()
            .json()
            .unwrap()
    }
    pub(super) fn snapshot(&self) -> golden_core::ui_sync::UiSnapshot {
        self.post(
            "/api/ui/snapshot",
            &UiSnapshotRequest {
                scope: UiSubscriptionScope::WholeGraph,
                cancel_active_edit_session: false,
            },
        )
    }
    pub(super) fn intent(&self, intent: &UiEditIntent) -> UiAck {
        self.post("/api/ui/intent", intent)
    }
    pub(super) fn upload(&self, contents: String) {
        let uploaded: UiProjectPathDto = self.post(
            "/api/ui/project-upload-load",
            &UiProjectUploadRequest {
                file_name: "p08-fixture.noisette".into(),
                contents,
                recover: false,
            },
        );
        let uploaded_path = PathBuf::from(uploaded.path).canonicalize().unwrap();
        assert!(uploaded_path.starts_with(self.evidence.canonicalize().unwrap()));
    }
    pub(super) fn websocket(&self) -> tungstenite::WebSocket<MaybeTlsStream<TcpStream>> {
        let mut request = format!("{}/api/ui/ws", self.base.replacen("http:", "ws:", 1))
            .into_client_request()
            .unwrap();
        let mut protocols = self
            .websocket_protocols
            .join(", ")
            .parse::<tungstenite::http::HeaderValue>()
            .unwrap();
        protocols.set_sensitive(true);
        request.headers_mut().insert("Sec-WebSocket-Protocol", protocols);
        let (mut socket, response) = tungstenite::connect(request).unwrap();
        assert_eq!(response.headers()["sec-websocket-protocol"], "golden-ui");
        if let MaybeTlsStream::Plain(stream) = socket.get_mut() {
            stream.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
        }
        socket
            .send(Message::Text(
                serde_json::to_string(&UiClientMessage::Hello {
                    protocol_version: UI_PROTOCOL_VERSION.into(),
                    client_instance_id: Some("p08-native-evidence".into()),
                })
                .unwrap()
                .into(),
            ))
            .unwrap();
        socket
    }
    pub(super) fn ws_intent(
        socket: &mut tungstenite::WebSocket<MaybeTlsStream<TcpStream>>,
        request_id: &str,
        intent: UiEditIntent,
    ) -> UiAck {
        socket
            .send(Message::Text(
                serde_json::to_string(&UiClientMessage::Intent {
                    request_id: request_id.into(),
                    intent: Box::new(intent),
                    include_self_events: false,
                })
                .unwrap()
                .into(),
            ))
            .unwrap();
        for _ in 0..32 {
            let message = socket.read().unwrap();
            if !message.is_text() {
                continue;
            }
            match serde_json::from_str::<UiServerMessage>(message.to_text().unwrap()).unwrap() {
                UiServerMessage::Hello { protocol_version, .. } => assert_eq!(protocol_version, UI_PROTOCOL_VERSION),
                UiServerMessage::Control { update } if update.request_id == request_id => {
                    if matches!(update.phase, UiControlPhase::Applied | UiControlPhase::Rejected) {
                        return update.acknowledgement.expect("terminal acknowledgement");
                    }
                }
                UiServerMessage::Error { message, .. } => panic!("WebSocket error: {message}"),
                _ => {}
            }
        }
        panic!("no terminal app-operation reply");
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        eprintln!("P08 native transport evidence: {}", self.evidence.display());
    }
}

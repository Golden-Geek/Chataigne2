use super::*;
use tokio_tungstenite::tungstenite::{self, Message, WebSocket};

struct Client(WebSocket<TcpStream>);

impl Client {
    fn connect(endpoint: SocketAddr) -> Self {
        let connection = TcpStream::connect(endpoint).unwrap();
        connection.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        let (socket, _) = tungstenite::client(format!("ws://{endpoint}/"), connection).unwrap();
        Self(socket)
    }

    fn control(&mut self, name: &str, data: Value, success: bool) {
        self.0.get_mut().set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        self.0
            .send(Message::text(json!({"COMMAND": name, "DATA": data}).to_string()))
            .unwrap();
        let frame = self.0.read().unwrap();
        let reply: Value = serde_json::from_str(frame.to_text().unwrap()).unwrap();
        assert_eq!(reply["COMMAND"], "RESULT");
        assert_eq!(reply["DATA"]["command"], name);
        assert_eq!(reply["DATA"]["ok"], success, "{reply}");
    }

    fn packet(&mut self, timeout: Duration) -> Option<OscPacket> {
        self.0.get_mut().set_read_timeout(Some(timeout)).unwrap();
        match self.0.read() {
            Ok(Message::Binary(bytes)) => {
                let (remaining, packet) = rosc::decoder::decode_udp(&bytes).unwrap();
                assert!(remaining.is_empty());
                Some(packet)
            }
            Err(tungstenite::Error::Io(error))
                if matches!(error.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut) =>
            {
                None
            }
            other => panic!("Expected binary OSC feedback or a timeout, got {other:?}"),
        }
    }
}

#[test]
fn listeners_receive_isolated_immediate_updates_and_bounded_bursts() {
    let host = Host::start();
    let initial = host.snapshot();
    let prefs = chosen(&initial, initial.root(), "preferences");
    let category = chosen(&initial, prefs.node_id, "oscquery");
    let port = chosen(&initial, category.node_id, "port");
    let mixer = chosen(&initial, initial.root(), "mixer");
    let gain = chosen(&initial, mixer.node_id, "gain").node_id;
    let muted = chosen(&initial, mixer.node_id, "muted").node_id;
    let fire = chosen(&initial, mixer.node_id, "fire").node_id;
    let reserved = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = reserved.local_addr().unwrap();
    drop(reserved);
    host.set(port.node_id, ParamValue::Int(endpoint.port().into()));
    host.enable(category.node_id, true);
    wait(|| http(endpoint, "/?HOST_INFO").is_ok());

    let mut first = Client::connect(endpoint);
    let mut second = Client::connect(endpoint);
    let read = Duration::from_secs(2);
    let silence = Duration::from_millis(65);
    first.control("LISTEN", json!("/mixer"), false);
    first.control("LISTEN", json!(["/mixer/gain", true]), true);
    assert_eq!(
        first.packet(read),
        Some(message("/mixer/gain", vec![OscType::Float(0.25)]))
    );
    first.control("LISTEN", json!("/mixer/fire"), true);
    second.control("LISTEN", json!({"address": "/mixer/muted", "sendFeedback": true}), true);
    assert_eq!(
        second.packet(read),
        Some(message("/mixer/muted", vec![OscType::Int(0)]))
    );
    assert!(first.packet(silence).is_none());

    let send_at = Instant::now();
    host.set(gain, ParamValue::Float(0.5));
    assert_eq!(
        first.packet(read),
        Some(message("/mixer/gain", vec![OscType::Float(0.5)]))
    );
    let delay = send_at.elapsed();
    assert!(
        delay < Duration::from_millis(140),
        "isolated feedback latency {delay:?}"
    );
    assert!(second.packet(silence).is_none());

    let set = |node, value| UiEditIntent::SetParam {
        node,
        value,
        behaviour: ParameterEventBehaviour::Append,
    };
    let mut burst = Vec::new();
    for index in 1..=24 {
        burst.push(set(gain, ParamValue::Float(f64::from(index) / 32.0)));
        if index % 6 == 0 {
            burst.push(set(fire, ParamValue::Trigger()));
        }
    }
    burst.push(set(muted, ParamValue::Bool(true)));
    let applied = host.runtime.apply_ui_transaction_batch(burst, None, true).unwrap();
    assert!(applied.transactions.iter().all(|result| result.acknowledgement.success));
    let mut frames = 0;
    let mut bundles = 0;
    let mut pulses = 0;
    let mut final_gain = None;
    while pulses < 4 || final_gain != Some(0.75) {
        let frame = first.packet(read).expect("burst feedback did not converge");
        frames += 1;
        assert!(frames <= 8, "burst delivered too many frames");
        let messages = match frame {
            OscPacket::Bundle(bundle) => {
                bundles += 1;
                assert_eq!(bundle.timetag, (0, 1).into());
                bundle.content
            }
            OscPacket::Message(_) => vec![frame],
        };
        for packet in messages {
            let OscPacket::Message(message) = packet else {
                panic!("Unexpected nested feedback bundle")
            };
            match (message.addr.as_str(), message.args.as_slice()) {
                ("/mixer/gain", [OscType::Float(value)]) => final_gain = Some(*value),
                ("/mixer/fire", [OscType::Inf]) => pulses += 1,
                other => panic!("Unsubscribed or malformed update: {other:?}"),
            }
        }
    }
    assert!(bundles >= 1);
    assert_eq!(pulses, 4);
    assert!(first.packet(silence).is_none());
    assert_eq!(
        second.packet(read),
        Some(message("/mixer/muted", vec![OscType::Int(1)]))
    );
    assert!(second.packet(silence).is_none());
    eprintln!(
        "isolated_feedback_ms={:.3} burst_frames={frames} burst_bundles={bundles} trigger_pulses={pulses}",
        delay.as_secs_f64() * 1000.0
    );

    second.control("IGNORE", json!("/mixer/muted"), true);
    host.set(muted, ParamValue::Bool(false));
    assert!(second.packet(silence).is_none());
    host.apply(UiEditIntent::PatchMeta {
        node: gain,
        patch: NodeMetaPatch {
            short_name: Some("level".into()),
            ..Default::default()
        }
        .into(),
    });
    host.set(gain, ParamValue::Float(0.625));
    assert!(first.packet(silence).is_none());
    first.control("LISTEN", json!(["/mixer/level", true]), true);
    assert_eq!(
        first.packet(read),
        Some(message("/mixer/level", vec![OscType::Float(0.625)]))
    );
    drop(first);
    let mut reconnect = Client::connect(endpoint);
    host.set(gain, ParamValue::Float(0.375));
    assert!(reconnect.packet(silence).is_none());
}

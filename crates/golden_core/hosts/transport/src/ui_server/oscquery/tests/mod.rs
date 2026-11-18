use super::*;
use crate::ui_server::{UiServerConfig, UiServerShutdown};
use golden_engine::{
    app::create_new_project_engine,
    application::ProductionRuntime,
    edit::{Edit, NodeTree},
    node::{Folder, NodeId, NodeMetaPatch},
    parameter::{ParamValue, Parameter, ParameterChangeCheck, ParameterEventBehaviour, RangeConstraint},
    ui_read_model::UiSnapshotCapture,
};
use golden_protocol::{UiEditIntent, UiEventKind, UiNodeDataDto, UiNodeDto, UiSubscriptionScope};
use rosc::{OscBundle, OscMessage, OscPacket, OscType};
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpListener, UdpSocket},
    sync::mpsc,
    thread,
};

golden_engine::define_node_enum!(
    enum FixtureNode {}
);
impl ProjectLifecycle for FixtureNode {
    fn application_display_name() -> &'static str {
        "OSC fixture"
    }
}

fn chosen<'a>(snapshot: &'a UiSnapshotCapture, parent: NodeId, declaration: &str) -> &'a UiNodeDto {
    snapshot
        .node(parent)
        .unwrap()
        .children
        .iter()
        .filter_map(|id| snapshot.node(*id))
        .find(|node| node.decl_id.0 == declaration)
        .unwrap()
}

struct Host {
    runtime: ProductionRuntime<FixtureNode>,
    shutdown: UiServerShutdown,
    done: mpsc::Receiver<io::Result<()>>,
}

impl Host {
    fn start() -> Self {
        let mut engine = create_new_project_engine::<FixtureNode>().unwrap();
        let mut gain = Parameter::new("Gain", ParamValue::Float(0.25), ParameterChangeCheck::ValueChange);
        gain.constraints.range = Some(RangeConstraint::Uniform {
            min: Some(0.0),
            max: Some(1.0),
        });
        let mut meter = Parameter::new("Meter", ParamValue::Float(0.5), ParameterChangeCheck::ValueChange);
        meter.read_only = true;
        let tree = NodeTree::new(Folder::new("Mixer"))
            .with_child(NodeTree::new(gain))
            .with_child(NodeTree::new(meter))
            .with_child(NodeTree::new(Parameter::new(
                "Muted",
                ParamValue::Bool(false),
                ParameterChangeCheck::ValueChange,
            )))
            .with_child(NodeTree::new(Folder::new("Empty")))
            .with_child(NodeTree::new(Folder::new("Same Name")))
            .with_child(NodeTree::new(Folder::new("Same Name")))
            .with_child(NodeTree::new(Parameter::new(
                "Fire",
                ParamValue::Trigger(),
                ParameterChangeCheck::ValueChange,
            )));
        engine.edits.push(Edit::AddNodeTree {
            tree,
            parent: engine.root,
            prev_sibling: None,
        });
        let shutdown = UiServerShutdown::default();
        let (ready, runtime) = mpsc::sync_channel(1);
        let (complete, done) = mpsc::sync_channel(1);
        let stop = shutdown.clone();
        thread::spawn(move || {
            let result = crate::run_with_ui_server_config_and_ready(
                engine,
                UiServerConfig {
                    bind_addr: "127.0.0.1:0".into(),
                    shutdown: Some(stop),
                    ..Default::default()
                },
                |runtime| {
                    ready.send(runtime).unwrap();
                    Ok(())
                },
            );
            let _ = complete.send(result);
        });
        Self {
            runtime: runtime.recv_timeout(Duration::from_secs(4)).unwrap(),
            shutdown,
            done,
        }
    }

    fn snapshot(&self) -> UiSnapshotCapture {
        self.runtime
            .read_model()
            .capture_snapshot(UiSubscriptionScope::WholeGraph)
    }

    fn apply(&self, intent: UiEditIntent) {
        let result = self.runtime.apply_ui_transaction(intent, None).unwrap();
        assert!(result.acknowledgement.success, "{:?}", result.acknowledgement);
    }

    fn set(&self, node: NodeId, value: ParamValue) {
        self.apply(UiEditIntent::SetParam {
            node,
            value,
            behaviour: ParameterEventBehaviour::Append,
        });
    }

    fn enable(&self, node: NodeId, enabled: bool) {
        self.apply(UiEditIntent::PatchMeta {
            node,
            patch: NodeMetaPatch {
                enabled: Some(enabled),
                ..Default::default()
            }
            .into(),
        });
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        self.shutdown.request();
    }
}

fn http(address: SocketAddr, target: &str) -> io::Result<(u16, Value)> {
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_millis(150))?;
    stream.set_read_timeout(Some(Duration::from_millis(400)))?;
    write!(
        stream,
        "GET {target} HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\r\n"
    )?;
    let mut wire = String::new();
    stream.read_to_string(&mut wire)?;
    let (header, body) = wire
        .split_once("\r\n\r\n")
        .ok_or_else(|| io::Error::other("Incomplete response"))?;
    let code = header
        .split_whitespace()
        .nth(1)
        .unwrap()
        .parse::<u16>()
        .map_err(io::Error::other)?;
    let body = if body.is_empty() {
        Value::Null
    } else {
        serde_json::from_str(body).map_err(io::Error::other)?
    };
    Ok((code, body))
}

fn wait(mut ready: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(4);
    while !ready() {
        assert!(Instant::now() < deadline, "timed out observing the OSCQuery listener");
        thread::sleep(Duration::from_millis(30));
    }
}

#[test]
fn shared_host_rebinds_and_serves_live_oscquery_from_the_golden_projection() {
    let host = Host::start();
    let initial = host.snapshot();
    let prefs = chosen(&initial, initial.root(), "preferences");
    let category = chosen(&initial, prefs.node_id, "oscquery");
    let port = chosen(&initial, category.node_id, "port");
    assert!(!category.meta.enabled);
    let UiNodeDataDto::Parameter { param } = &port.data else {
        panic!("expected typed OSCQuery Port")
    };
    assert_eq!(param.value, ParamValue::Int(42000));

    let occupied = TcpListener::bind("127.0.0.1:0").unwrap();
    let first = occupied.local_addr().unwrap();
    host.set(port.node_id, ParamValue::Int(first.port().into()));
    host.enable(category.node_id, true);
    // Leave the collision alive beyond one settings observation, then release it and let
    // the ordinary retry bind without another settings edit or host restart.
    thread::sleep(Duration::from_millis(275));
    drop(occupied);
    wait(|| http(first, "/?HOST_INFO").is_ok());
    let (code, info) = http(first, "/?HOST_INFO").unwrap();
    assert_eq!(code, 200);
    assert_eq!(info["NAME"], "OSC fixture");
    assert_eq!(info["OSC_IP"], "127.0.0.1");
    assert_eq!(info["OSC_PORT"], first.port());
    assert_eq!(info["OSC_TRANSPORT"], "UDP");
    assert_eq!(info["EXTENSIONS"]["VALUE"], true);
    assert!(info["EXTENSIONS"].get("LISTEN").is_none());

    let mixer = chosen(&initial, initial.root(), "mixer");
    let gain = chosen(&initial, mixer.node_id, "gain");
    let (code, graph) = http(first, "/").unwrap();
    assert_eq!(code, 200);
    let children = &graph["CONTENTS"]["mixer"]["CONTENTS"];
    assert_eq!(children["gain"]["FULL_PATH"], "/mixer/gain");
    assert_eq!(children["gain"]["TYPE"], "f");
    assert_eq!(children["gain"]["ACCESS"], 3);
    assert_eq!(children["meter"]["ACCESS"], 1);
    assert_eq!(children["fire"]["ACCESS"], 2);
    assert_eq!(children["gain"]["VALUE"], json!([0.25]));
    assert_eq!(children["empty"]["CONTENTS"], json!({}));
    let fallbacks = children
        .as_object()
        .unwrap()
        .keys()
        .filter(|key| key.starts_with('~'))
        .collect::<Vec<_>>();
    assert_eq!(fallbacks.len(), 2, "duplicate short names get unique addresses");
    for key in fallbacks {
        let address = format!("/mixer/{key}");
        assert_eq!(http(first, &address).unwrap().1["FULL_PATH"], address);
    }
    assert_eq!(
        http(first, "/mixer/gain?RANGE&TYPE").unwrap(),
        (200, json!({"RANGE": [{"MIN": 0.0, "MAX": 1.0}], "TYPE": "f"}))
    );
    assert_eq!(http(first, "/mixer/fire?VALUE").unwrap().0, 204);
    assert_eq!(http(first, "/mixer/absent").unwrap().0, 404);
    assert_eq!(http(first, "/mixer/gain?BOGUS").unwrap().0, 400);
    host.set(gain.node_id, ParamValue::Float(0.75));
    assert_eq!(http(first, "/mixer/gain?VALUE").unwrap().1, json!({"VALUE": [0.75]}));

    let reservation = TcpListener::bind("127.0.0.1:0").unwrap();
    let second = reservation.local_addr().unwrap();
    let udp_collision = UdpSocket::bind(second).unwrap();
    drop(reservation);
    host.set(port.node_id, ParamValue::Int(second.port().into()));
    wait(|| TcpStream::connect(first).is_err());
    assert!(TcpStream::connect(second).is_err());
    drop(udp_collision);
    wait(|| http(second, "/mixer/gain?VALUE").is_ok());
    assert!(TcpStream::connect(first).is_err());
    host.enable(category.node_id, false);
    wait(|| TcpStream::connect(second).is_err());
    host.enable(category.node_id, true);
    wait(|| http(second, "/?HOST_INFO").is_ok());
    host.shutdown.request();
    host.done.recv_timeout(Duration::from_secs(3)).unwrap().unwrap();
    assert!(TcpStream::connect(second).is_err());
}

fn message(address: &str, args: Vec<OscType>) -> OscPacket {
    OscPacket::Message(OscMessage {
        addr: address.into(),
        args,
    })
}

fn send(sender: &UdpSocket, address: SocketAddr, packet: OscPacket) {
    let bytes = rosc::encoder::encode(&packet).unwrap();
    assert_eq!(sender.send_to(&bytes, address).unwrap(), bytes.len());
}

#[test]
fn osc_datagrams_use_golden_validation_events_and_undo() {
    let host = Host::start();
    let snapshot = host.snapshot();
    let prefs = chosen(&snapshot, snapshot.root(), "preferences");
    let category = chosen(&snapshot, prefs.node_id, "oscquery");
    let port = chosen(&snapshot, category.node_id, "port");
    let mixer = chosen(&snapshot, snapshot.root(), "mixer");
    let fire = chosen(&snapshot, mixer.node_id, "fire");
    let reservation = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = reservation.local_addr().unwrap();
    drop(reservation);
    let sender = UdpSocket::bind("127.0.0.1:0").unwrap();
    host.set(port.node_id, ParamValue::Int(address.port().into()));
    host.enable(category.node_id, true);
    wait(|| http(address, "/?HOST_INFO").is_ok());

    let value = |path| http(address, path).unwrap().1["VALUE"][0].clone();
    send(&sender, address, message("/mixer/gain", vec![OscType::Float(2.0)]));
    wait(|| value("/mixer/gain?VALUE") == 1.0);
    host.apply(UiEditIntent::Undo);
    assert_eq!(value("/mixer/gain?VALUE"), 0.25);
    host.apply(UiEditIntent::Redo);
    assert_eq!(value("/mixer/gain?VALUE"), 1.0);

    let before = host.snapshot().revision();
    send(
        &sender,
        address,
        OscPacket::Bundle(OscBundle {
            timetag: input::IMMEDIATE,
            content: vec![
                message("/mixer/gain", vec![OscType::Float(f32::NAN)]),
                message("/mixer/meter", vec![OscType::Float(0.75)]),
                message("/mixer/gain", vec![OscType::String("wrong type".into())]),
                message("/mixer/fire", vec![]),
                message("/mixer/muted", vec![OscType::Bool(true)]),
            ],
        }),
    );
    wait(|| value("/mixer/muted?VALUE") == 1);
    assert_eq!(value("/mixer/gain?VALUE"), 1.0);
    assert_eq!(value("/mixer/meter?VALUE"), 0.5);
    let replay = host
        .runtime
        .read_model()
        .replay(Some(before), UiSubscriptionScope::WholeGraph);
    assert!(replay.events.iter().any(|event| matches!(event.kind,
        UiEventKind::ParamChanged { param, new_value: ParamValue::Trigger(), .. } if param == fire.node_id
    )));
    host.enable(category.node_id, false);
    wait(|| TcpStream::connect(address).is_err());
    let _released = UdpSocket::bind(address).unwrap();
}

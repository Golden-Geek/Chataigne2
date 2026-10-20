use super::*;
use golden_core::{
    node::{Folder, NodeUuid},
    process_ctx::ProcessTreeNodeSnapshot,
    script::ScriptMethodCall,
};
use std::collections::HashMap;

mod effects;
mod preparation;

struct Fixture {
    engine: crate::app::AppEngine,
    template: ProcessTreeNodeSnapshot,
    nodes: HashMap<NodeId, ProcessTreeNodeSnapshot>,
    tails: HashMap<NodeId, NodeId>,
    module: NodeId,
    outputs: NodeId,
    values: NodeId,
    value: NodeId,
}

impl Fixture {
    fn new() -> Self {
        let engine = crate::app::AppEngine::new(Folder::new("producer").into());
        let template = engine.process_tree_snapshot().node(engine.root).unwrap().clone();
        let mut fixture = Self {
            engine,
            template,
            nodes: HashMap::new(),
            tails: HashMap::new(),
            module: NodeId(0),
            outputs: NodeId(0),
            values: NodeId(0),
            value: NodeId(0),
        };
        fixture.module = fixture.add(None, "module", "osc_module", None);
        let connection = fixture.add(Some(fixture.module), "connection", "folder", None);
        fixture.add(
            Some(connection),
            "log_outgoing",
            "parameter",
            Some(ParamValue::Bool(true)),
        );
        fixture.outputs = fixture.add(Some(connection), "outputs", "osc_output_manager", None);
        fixture.values = fixture.add(Some(fixture.module), "values", "folder", None);
        fixture.value = fixture.add(Some(fixture.values), "gain", "parameter", Some(ParamValue::Float(0.25)));
        fixture
    }

    fn add(&mut self, parent: Option<NodeId>, decl: &str, kind: &str, value: Option<ParamValue>) -> NodeId {
        let id = NodeId(self.nodes.len() as u64 + 1);
        let mut node = self.template.clone();
        node.id = id;
        node.uuid = NodeUuid(uuid::Uuid::from_u128(id.0 as u128));
        node.parent = parent;
        node.first_child = None;
        node.next_sibling = None;
        node.child_count = 0;
        node.decl_id = decl.into();
        node.label = decl.into();
        node.node_type = kind.into();
        node.param_value = value;
        node.enabled = true;
        self.nodes.insert(id, node);
        if let Some(parent) = parent {
            if let Some(tail) = self.tails.insert(parent, id) {
                self.nodes.get_mut(&tail).unwrap().next_sibling = Some(id);
            } else {
                self.nodes.get_mut(&parent).unwrap().first_child = Some(id);
            }
            self.nodes.get_mut(&parent).unwrap().child_count += 1;
        }
        id
    }

    fn output(&mut self, parent: NodeId, host: &str, port: i32) -> (NodeId, NodeId) {
        let output = self.add(Some(parent), "destination", "osc_output", None);
        let host = self.add(
            Some(output),
            "remote_host",
            "parameter",
            Some(ParamValue::Str(host.into())),
        );
        self.add(Some(output), "remote_port", "parameter", Some(ParamValue::Int(port)));
        (output, host)
    }

    fn snapshot(&self) -> Arc<ProcessTreeSnapshot> {
        Arc::new(ProcessTreeSnapshot::new(self.module, self.nodes.clone()))
    }

    fn producer(&self) -> NodeWaker {
        self.engine.node_waker(self.engine.root).unwrap()
    }

    fn request(&self) -> OutboundRequest {
        OutboundRequest::values(
            &self.snapshot(),
            self.module,
            self.values,
            self.value,
            self.producer(),
            true,
        )
        .unwrap()
    }

    fn script(&self, values: Vec<ParamValue>) -> Result<OutboundRequest, QueueError> {
        let call = ScriptMethodCall::prepare("sendOSC", values).unwrap();
        OutboundRequest::script(&self.snapshot(), self.module, call.args().clone(), self.producer())
    }
}

fn prepare(request: OutboundRequest) -> Result<PreparedSend, String> {
    let mut preparation = Preparation::new(request)?;
    for _ in 0..=MAX_EDGES / PREPARATION_TURN {
        if preparation.service()? {
            return preparation.finish();
        }
    }
    panic!("preparation did not enforce its edge budget");
}

fn decode(bytes: &[u8]) -> rosc::OscMessage {
    match rosc::decoder::decode_udp(bytes).unwrap().1 {
        rosc::OscPacket::Message(message) => message,
        other => panic!("unexpected packet: {other:?}"),
    }
}

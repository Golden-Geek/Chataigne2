use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use golden_model::Revision;

use super::*;

#[derive(Clone)]
struct TestDomain;

#[derive(Clone, Debug)]
enum TestNode {
    Source,
    Pass,
    Sink,
    Tracked(TrackedPayload),
}

#[derive(Debug)]
struct TrackedPayload {
    value: usize,
    clones: Arc<AtomicUsize>,
}

impl Clone for TrackedPayload {
    fn clone(&self) -> Self {
        self.clones.fetch_add(1, Ordering::Relaxed);
        Self {
            value: self.value,
            clones: Arc::clone(&self.clones),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TestPort(&'static str);

impl GraphDomain for TestDomain {
    type GraphData = ();
    type NodeData = TestNode;
    type PortData = TestPort;
    type EdgeData = ();

    fn node_ports(&self, node: &Self::NodeData, _graph: &GraphDocument<Self>) -> Vec<PortDescriptor<Self::PortData>> {
        let input = PortDescriptor {
            id: input_port(),
            direction: PortDirection::Input,
            data: TestPort("value"),
        };
        let output = PortDescriptor {
            id: output_port(),
            direction: PortDirection::Output,
            data: TestPort("value"),
        };
        match node {
            TestNode::Source => vec![output],
            TestNode::Sink => vec![input],
            TestNode::Pass | TestNode::Tracked(_) => vec![input, output],
        }
    }

    fn validate_connection(
        &self,
        _graph: &GraphDocument<Self>,
        from: PortRef,
        to: PortRef,
        _edge: &Self::EdgeData,
    ) -> Result<(), GraphDiagnostic> {
        if from.node == to.node {
            Err(GraphDiagnostic::error("self_edge", "self edges are forbidden"))
        } else {
            Ok(())
        }
    }
}

struct TestProtocolAdapter;

impl GraphProtocolAdapter<TestDomain> for TestProtocolAdapter {
    fn domain_id(&self) -> &str {
        "test"
    }

    fn domain_schema_version(&self) -> u32 {
        3
    }

    fn encode_graph_data(&self, _data: &()) -> serde_json::Value {
        serde_json::Value::Null
    }

    fn encode_node_data(&self, data: &TestNode) -> serde_json::Value {
        serde_json::Value::String(
            match data {
                TestNode::Source => "source",
                TestNode::Pass => "pass",
                TestNode::Sink => "sink",
                TestNode::Tracked(_) => "tracked",
            }
            .to_owned(),
        )
    }

    fn encode_edge_data(&self, _data: &()) -> serde_json::Value {
        serde_json::Value::Null
    }
}

fn input_port() -> GraphPortId {
    GraphPortId::from_entity(golden_model::EntityId::from_uuid(uuid::Uuid::from_u128(1)))
}

fn output_port() -> GraphPortId {
    GraphPortId::from_entity(golden_model::EntityId::from_uuid(uuid::Uuid::from_u128(2)))
}

fn node(data: TestNode) -> GraphNode<TestDomain> {
    GraphNode {
        id: GraphNodeId::new(),
        data,
    }
}

fn edge(from: GraphNodeId, to: GraphNodeId) -> GraphEdge<TestDomain> {
    GraphEdge {
        id: GraphEdgeId::new(),
        from: PortRef {
            node: from,
            port: output_port(),
        },
        to: PortRef {
            node: to,
            port: input_port(),
        },
        data: (),
    }
}

#[test]
fn transactions_emit_precise_changes_and_update_indexes() {
    let mut graph = GraphDocument::new(GraphId::new(), TestDomain, ());
    let source = node(TestNode::Source);
    let sink = node(TestNode::Sink);
    let connection = edge(source.id, sink.id);
    let mut transaction = GraphTransaction::new(Revision::ZERO);
    transaction.push(GraphOperation::InsertNode {
        node: source.clone(),
        presentation: None,
    });
    transaction.push(GraphOperation::InsertNode {
        node: sink.clone(),
        presentation: None,
    });
    transaction.push(GraphOperation::Connect {
        edge: connection.clone(),
    });

    let commit = graph.apply(transaction).unwrap();
    assert_eq!(
        commit.change_set.changes,
        [
            GraphChange::NodeInserted(source.id),
            GraphChange::NodeInserted(sink.id),
            GraphChange::EdgeInserted(connection.id),
        ]
    );
    assert_eq!(graph.outgoing_edges(source.id).count(), 1);
    assert_eq!(graph.incoming_edges(sink.id).count(), 1);
    graph.assert_invariants().unwrap();
}

#[test]
fn failed_transactions_roll_back_every_prior_operation() {
    let mut graph = GraphDocument::new(GraphId::new(), TestDomain, ());
    let source = node(TestNode::Source);
    let mut transaction = GraphTransaction::new(Revision::ZERO);
    transaction.push(GraphOperation::InsertNode {
        node: source.clone(),
        presentation: None,
    });
    transaction.push(GraphOperation::Connect {
        edge: edge(source.id, source.id),
    });
    assert!(graph.apply(transaction).is_err());
    assert_eq!(graph.revision(), Revision::ZERO);
    assert_eq!(graph.nodes().len(), 0);
    assert_eq!(graph.edges().len(), 0);
}

#[test]
fn removing_a_node_removes_all_incident_edges_atomically() {
    let mut graph = GraphDocument::new(GraphId::new(), TestDomain, ());
    let source = node(TestNode::Source);
    let pass = node(TestNode::Pass);
    let sink = node(TestNode::Sink);
    let first = edge(source.id, pass.id);
    let second = edge(pass.id, sink.id);
    let mut setup = GraphTransaction::new(Revision::ZERO);
    for record in [source.clone(), pass.clone(), sink.clone()] {
        setup.push(GraphOperation::InsertNode {
            node: record,
            presentation: None,
        });
    }
    setup.push(GraphOperation::Connect { edge: first });
    setup.push(GraphOperation::Connect { edge: second });
    graph.apply(setup).unwrap();

    let mut remove = GraphTransaction::new(graph.revision());
    remove.push(GraphOperation::RemoveNode { node: pass.id });
    let commit = graph.apply(remove).unwrap();
    assert_eq!(graph.nodes().len(), 2);
    assert_eq!(graph.edges().len(), 0);
    assert_eq!(
        commit
            .change_set
            .changes
            .iter()
            .filter(|change| matches!(change, GraphChange::EdgeRemoved(_)))
            .count(),
        2
    );
}

#[test]
fn traversal_is_stable_and_reports_cycles_as_components() {
    let mut graph = GraphDocument::new(GraphId::new(), TestDomain, ());
    let a = node(TestNode::Pass);
    let b = node(TestNode::Pass);
    let c = node(TestNode::Pass);
    let mut setup = GraphTransaction::new(Revision::ZERO);
    for record in [a.clone(), b.clone(), c.clone()] {
        setup.push(GraphOperation::InsertNode {
            node: record,
            presentation: None,
        });
    }
    setup.push(GraphOperation::Connect { edge: edge(a.id, b.id) });
    setup.push(GraphOperation::Connect { edge: edge(b.id, c.id) });
    graph.apply(setup).unwrap();
    assert_eq!(stable_topological_order(&graph).unwrap(), [a.id, b.id, c.id]);

    let mut cycle = GraphTransaction::new(graph.revision());
    cycle.push(GraphOperation::Connect { edge: edge(c.id, a.id) });
    graph.apply(cycle).unwrap();
    assert!(stable_topological_order(&graph).is_err());
    let mut expected_component = vec![a.id, b.id, c.id];
    expected_component.sort_unstable();
    assert_eq!(strongly_connected_components(&graph), [expected_component]);
}

#[test]
fn one_node_replacement_does_not_clone_the_document() {
    let clones = Arc::new(AtomicUsize::new(0));
    let mut graph = GraphDocument::new(GraphId::new(), TestDomain, ());
    let mut setup = GraphTransaction::new(Revision::ZERO);
    let mut ids = Vec::new();
    for value in 0..10_000 {
        let record = node(TestNode::Tracked(TrackedPayload {
            value,
            clones: Arc::clone(&clones),
        }));
        ids.push(record.id);
        setup.push(GraphOperation::InsertNode {
            node: record,
            presentation: None,
        });
    }
    graph.apply(setup).unwrap();
    clones.store(0, Ordering::Relaxed);

    let mut replace = GraphTransaction::new(graph.revision());
    replace.push(GraphOperation::ReplaceNode {
        node: ids[5_000],
        data: TestNode::Tracked(TrackedPayload {
            value: usize::MAX,
            clones: Arc::clone(&clones),
        }),
    });
    graph.apply(replace).unwrap();
    assert_eq!(clones.load(Ordering::Relaxed), 0);
    let TestNode::Tracked(payload) = &graph.node(ids[5_000]).unwrap().data else {
        panic!("replaced node should retain tracked payload data");
    };
    assert_eq!(payload.value, usize::MAX);
}

#[test]
fn deterministic_mutation_sequences_preserve_topology_properties() {
    let mut graph = GraphDocument::new(GraphId::new(), TestDomain, ());
    let records = (0..256).map(|_| node(TestNode::Pass)).collect::<Vec<_>>();
    let mut insert = GraphTransaction::new(Revision::ZERO);
    for record in records.iter().cloned() {
        insert.push(GraphOperation::InsertNode {
            node: record,
            presentation: None,
        });
    }
    graph.apply(insert).unwrap();

    let mut seed = 0x5eed_u64;
    let mut connect = GraphTransaction::new(graph.revision());
    for _ in 0..512 {
        seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        let from = (seed as usize) % records.len();
        seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        let mut to = (seed as usize) % records.len();
        if from == to {
            to = (to + 1) % records.len();
        }
        connect.push(GraphOperation::Connect {
            edge: edge(records[from].id, records[to].id),
        });
    }
    graph.apply(connect).unwrap();

    let mut remove = GraphTransaction::new(graph.revision());
    for record in records.iter().step_by(4) {
        remove.push(GraphOperation::RemoveNode { node: record.id });
    }
    graph.apply(remove).unwrap();
    graph.assert_invariants().unwrap();
    assert!(
        graph
            .edges()
            .all(|edge| { graph.node(edge.from.node).is_some() && graph.node(edge.to.node).is_some() })
    );
}

#[test]
fn presentation_operations_participate_in_transaction_rollback() {
    let mut graph = GraphDocument::new(GraphId::new(), TestDomain, ());
    let comment_id = golden_model::EntityId::new();
    let mut transaction = GraphTransaction::new(Revision::ZERO);
    transaction.push(GraphOperation::UpsertComment(GraphComment {
        id: comment_id,
        text: "note".to_owned(),
        position: Point::new(1.0, 2.0).unwrap(),
        size: Size::new(3.0, 4.0).unwrap(),
    }));
    transaction.push(GraphOperation::UpsertGroup(GraphGroup {
        id: golden_model::EntityId::new(),
        label: "invalid".into(),
        nodes: [GraphNodeId::new()].into_iter().collect(),
    }));
    assert!(graph.apply(transaction).is_err());
    assert!(graph.presentation().comments.is_empty());
    assert!(graph.presentation().groups.is_empty());
    assert_eq!(graph.revision(), Revision::ZERO);
}

#[test]
fn protocol_envelopes_require_an_explicit_domain_adapter() {
    let mut graph = GraphDocument::new(GraphId::new(), TestDomain, ());
    let source = node(TestNode::Source);
    let mut transaction = GraphTransaction::new(Revision::ZERO);
    transaction.push(GraphOperation::InsertNode {
        node: source,
        presentation: None,
    });
    graph.apply(transaction).unwrap();

    let dto = GraphDocumentDto::from_document(&graph, &TestProtocolAdapter);
    assert_eq!(dto.envelope_version, GRAPH_ENVELOPE_VERSION);
    assert_eq!(dto.domain_id, "test");
    assert_eq!(dto.domain_schema_version, 3);
    assert_eq!(dto.nodes.len(), 1);
}

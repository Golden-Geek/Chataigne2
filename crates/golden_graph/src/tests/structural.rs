use super::*;
use crate::{GraphDiagnostic, GraphDocumentData, GraphEdgeId, GraphNodeId, PortSchema};

type DocumentData = GraphDocumentData<TestGraphData, TestNodeData, TestEdgeData>;

fn add_group(data: &mut DocumentData, nodes: &[GraphNodeId]) -> GraphGroupId {
    let id = GraphGroupId::new();
    data.presentation.groups.insert(
        id,
        GraphGroup {
            id,
            label: "group".into(),
            nodes: nodes.iter().copied().collect(),
            position: [0.0; 2],
            size: [100.0; 2],
        },
    );
    id
}

fn fixture() -> (TestDocument, Vec<GraphNodeId>, Vec<GraphEdgeId>, [GraphGroupId; 3]) {
    let mut graph = GraphDocument::new(TestGraphData::default());
    let nodes = (0..6)
        .map(|n| node(TestNodeKind::PassThrough, &format!("node-{n}")))
        .collect::<Vec<_>>();
    let mut tx = GraphTransaction::for_document(&graph);
    for node in &nodes {
        tx.insert_node(node.clone(), Some(NodePresentation::default()));
    }
    let mut edges = Vec::new();
    for pair in nodes.windows(2) {
        let edge = edge(&pair[0], &pair[1]);
        edges.push(edge.id);
        tx.connect(edge);
    }
    tx.commit(&mut graph, &TestDomain).unwrap();
    let ids = nodes.iter().map(|node| node.id).collect::<Vec<_>>();
    let mut data = GraphDocumentData::from_document(&graph);
    let groups = [
        add_group(&mut data, &[ids[0], ids[1], ids[3]]),
        add_group(&mut data, &[ids[1], ids[2]]),
        add_group(&mut data, &[ids[4], ids[5]]),
    ];
    for _ in 0..256 {
        add_group(&mut data, &[ids[4], ids[5]]);
    }
    (data.into_document().unwrap(), ids, edges, groups)
}

fn remove_selection(
    graph: &TestDocument,
    nodes: &[GraphNodeId],
) -> GraphTransaction<TestGraphData, TestNodeData, TestEdgeData> {
    let mut tx = GraphTransaction::for_document(graph);
    for node in nodes {
        tx.remove_node(*node);
    }
    tx
}

#[test]
fn selection_delete_updates_only_incident_edges_and_member_groups_once() {
    let (mut graph, nodes, edges, groups) = fixture();
    let before = graph.revision();
    let untouched = graph.presentation().groups[&groups[2]].clone();
    let result = remove_selection(&graph, &nodes[1..4])
        .commit(&mut graph, &TestDomain)
        .unwrap();
    let changed = &result.delta.changes;
    assert_eq!(changed.removed_nodes, nodes[1..4].iter().copied().collect());
    assert_eq!(changed.removed_edges, edges[..4].iter().copied().collect());
    assert_eq!(changed.presentation_nodes, nodes[1..4].iter().copied().collect());
    assert_eq!(changed.presentation_groups, BTreeSet::from([groups[0], groups[1]]));
    assert_eq!(
        graph.presentation().groups[&groups[0]].nodes,
        BTreeSet::from([nodes[0]])
    );
    assert!(graph.presentation().groups[&groups[1]].nodes.is_empty());
    assert_eq!(graph.presentation().groups[&groups[2]], untouched);
    assert_eq!(
        graph.nodes().map(|node| node.id).collect::<Vec<_>>(),
        [nodes[0], nodes[4], nodes[5]]
    );
    assert_eq!(graph.edges().map(|edge| edge.id).collect::<Vec<_>>(), [edges[4]]);
    assert_eq!(graph.outgoing_edges(nodes[0]).count(), 0);
    assert_eq!(graph.incoming_edges(nodes[4]).count(), 0);
    assert_eq!(graph.outgoing_edges(nodes[4]).collect::<Vec<_>>(), [edges[4]]);
    assert_eq!(result.delta.to.sequence, before.sequence + 1);
    assert_eq!(result.delta.to.topology, before.topology + 1);
    assert_eq!(result.delta.to.payload, before.payload + 1);
    assert_eq!(result.delta.to.presentation, before.presentation + 1);
    assert_eq!(graph, GraphDocumentData::from_document(&graph).into_document().unwrap());
}

struct RejectRemoval;

impl GraphDomain for RejectRemoval {
    type GraphData = TestGraphData;
    type NodeData = TestNodeData;
    type EdgeData = TestEdgeData;
    type PortData = crate::testkit::TestPortData;

    fn domain_id(&self) -> &'static str {
        TestDomain.domain_id()
    }
    fn schema_version(&self) -> u32 {
        TestDomain.schema_version()
    }
    fn node_ports(&self, node: &TestNodeData, graph: &TestDocument) -> PortSchema<Self::PortData> {
        TestDomain.node_ports(node, graph)
    }
    fn validate_connection(&self, graph: &TestDocument, from: PortRef, to: PortRef) -> Result<(), GraphDiagnostic> {
        TestDomain.validate_connection(graph, from, to)
    }
    fn validate_graph(&self, _: &TestDocument, _: &crate::GraphChangeSet) -> Vec<GraphDiagnostic> {
        vec![GraphDiagnostic::error("rejected", "reject after structural edits")]
    }
}

#[test]
fn validation_rejection_restores_membership_indexes_and_the_next_selection_delete() {
    let (mut graph, nodes, _, groups) = fixture();
    let before = graph.clone();
    let encoded = serde_json::to_string(&GraphDocumentData::from_document(&graph)).unwrap();
    assert!(
        remove_selection(&graph, &nodes[1..4])
            .commit(&mut graph, &RejectRemoval)
            .is_err()
    );
    assert_eq!(graph, before);
    assert_eq!(
        serde_json::to_string(&GraphDocumentData::from_document(&graph)).unwrap(),
        encoded
    );
    let result = remove_selection(&graph, &nodes[1..4])
        .commit(&mut graph, &TestDomain)
        .unwrap();
    assert_eq!(
        result.delta.changes.presentation_groups,
        BTreeSet::from([groups[0], groups[1]])
    );
    assert_eq!(graph, GraphDocumentData::from_document(&graph).into_document().unwrap());
}

#[test]
fn loading_rejects_group_membership_for_a_missing_node() {
    let (graph, _, _, _) = fixture();
    let mut data = GraphDocumentData::from_document(&graph);
    add_group(&mut data, &[GraphNodeId::new()]);
    assert!(data.into_document().is_err());
}

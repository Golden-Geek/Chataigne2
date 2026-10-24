#![cfg(feature = "testkit")]

use std::{hint::black_box, time::Instant};

use golden_graph::{
    GraphDocument, GraphDocumentData, GraphEdge, GraphEdgeId, GraphGroup, GraphGroupId, GraphNode, GraphNodeId,
    GraphTransaction, NodePresentation, PortRef,
    testkit::{TestDomain, TestEdgeData, TestGraphData, TestNodeData, TestNodeKind},
};
use serde_json::{Value, json};
use uuid::Uuid;

type Document = GraphDocument<TestGraphData, TestNodeData, TestEdgeData>;

struct Fixture {
    document: Document,
    selected: Vec<GraphNodeId>,
}

impl Fixture {
    fn new(count: usize) -> Self {
        let mut data = GraphDocumentData::from_document(&Document::new(TestGraphData::default()));
        for index in 0..count {
            let id = node_id(index);
            data.nodes.insert(
                id,
                GraphNode {
                    id,
                    data: TestNodeData {
                        kind: TestNodeKind::PassThrough,
                        label: format!("node-{index}"),
                    },
                },
            );
            data.presentation.nodes.insert(
                id,
                NodePresentation {
                    position: [index as f64, 0.0],
                    ..Default::default()
                },
            );
            if index != 0 {
                let id = GraphEdgeId::from_uuid(Uuid::from_u128(index as u128));
                data.edges.insert(
                    id,
                    GraphEdge {
                        id,
                        from: PortRef::new(node_id(index - 1), TestDomain::output_port()),
                        to: PortRef::new(node_id(index), TestDomain::input_port()),
                        data: TestEdgeData,
                    },
                );
            }
        }
        for index in 0..count.div_ceil(16) {
            let id = GraphGroupId::from_uuid(Uuid::from_u128(index as u128 + 1));
            data.presentation.groups.insert(
                id,
                GraphGroup {
                    id,
                    label: format!("group-{index}"),
                    nodes: (index * 16..((index + 1) * 16).min(count)).map(node_id).collect(),
                    position: [0.0; 2],
                    size: [10.0; 2],
                },
            );
        }
        let selected = (0..32).map(|index| node_id(index * count / 32 + count / 64)).collect();
        Self {
            document: data.into_document().unwrap(),
            selected,
        }
    }
}

fn node_id(index: usize) -> GraphNodeId {
    GraphNodeId::from_uuid(Uuid::from_u128(index as u128 + 1))
}

#[derive(Clone, Copy)]
enum Action {
    Move,
    Delete,
    RollbackDelete,
}

fn execute(document: &mut Document, selected: &[GraphNodeId], action: Action) -> u64 {
    let before = document.revision();
    let nodes = document.nodes().len();
    let edges = document.edges().len();
    let started = Instant::now();
    let mut transaction = GraphTransaction::for_document(document);
    for id in selected {
        match action {
            Action::Move => transaction.set_node_presentation(
                *id,
                Some(NodePresentation {
                    position: [500.0, 250.0],
                    ..Default::default()
                }),
            ),
            Action::Delete | Action::RollbackDelete => transaction.remove_node(*id),
        }
    }
    if matches!(action, Action::RollbackDelete) {
        // All selected edits must be reversed inside the timed commit, after the prefix ran.
        transaction.remove_node(GraphNodeId::from_uuid(Uuid::nil()));
    }
    let result = black_box(transaction.commit(document, &TestDomain));
    let nanos = u64::try_from(started.elapsed().as_nanos()).unwrap();

    match action {
        Action::Move => {
            let delta = result.unwrap().delta;
            assert_eq!(delta.changes.presentation_nodes.len(), selected.len());
            assert_eq!(delta.to.topology, before.topology);
            assert_eq!(delta.to.payload, before.payload);
            assert_eq!(delta.to.presentation, before.presentation + 1);
            for id in selected {
                assert_eq!(document.presentation().nodes[id].position, [500.0, 250.0]);
            }
        }
        Action::Delete => {
            let delta = result.unwrap().delta;
            assert_eq!(delta.changes.removed_nodes.len(), selected.len());
            assert_eq!(delta.changes.removed_edges.len(), selected.len() * 2);
            assert_eq!(delta.changes.presentation_groups.len(), selected.len());
            assert_eq!(document.nodes().len(), nodes - selected.len());
            assert_eq!(document.edges().len(), edges - selected.len() * 2);
            assert_eq!(delta.to.sequence, before.sequence + 1);
        }
        Action::RollbackDelete => {
            assert!(result.is_err());
            assert_eq!(document.revision(), before);
            assert_eq!(document.nodes().len(), nodes);
            assert_eq!(document.edges().len(), edges);
            for id in selected {
                assert!(document.node(*id).is_some());
                assert_eq!(document.incoming_edges(*id).count(), 1);
                assert_eq!(document.outgoing_edges(*id).count(), 1);
            }
        }
    }
    nanos
}

#[test]
fn selection_edits_rollback_and_reload_preserve_public_document_contract() {
    let fixture = Fixture::new(1024);
    let mut document = fixture.document.clone();
    execute(&mut document, &fixture.selected, Action::RollbackDelete);
    assert_eq!(document, fixture.document);
    execute(&mut document, &fixture.selected, Action::Move);
    let moved = GraphDocumentData::from_document(&document).into_document().unwrap();
    assert_eq!(moved, document);
    execute(&mut document, &fixture.selected, Action::Delete);
    let deleted = GraphDocumentData::from_document(&document).into_document().unwrap();
    assert_eq!(deleted, document);
    for id in fixture.selected {
        assert!(document.node(id).is_none());
        assert!(!document.presentation().nodes.contains_key(&id));
        assert!(
            document
                .presentation()
                .groups
                .values()
                .all(|group| !group.nodes.contains(&id))
        );
    }
}

#[test]
#[ignore = "explicit fixed-delta measurement; retain raw output and machine/source identity"]
fn measure_fixed_graph_edits() {
    let mut rows = Vec::<Value>::new();
    for count in [1_000, 10_000, 50_000, 100_000] {
        let fixture = Fixture::new(count);
        for (name, action, selected_count) in [
            ("move_1", Action::Move, 1),
            ("move_32", Action::Move, 32),
            ("delete_1", Action::Delete, 1),
            ("delete_32", Action::Delete, 32),
            ("rollback_delete_32", Action::RollbackDelete, 32),
        ] {
            let mut samples = Vec::new();
            for index in 0..34 {
                // Fresh identical state for every sample. Clone/drop and all fixture work
                // are outside timing; only public transaction construction/commit is timed.
                let mut document = fixture.document.clone();
                let elapsed = execute(&mut document, &fixture.selected[..selected_count], action);
                if matches!(action, Action::RollbackDelete) {
                    assert_eq!(document, fixture.document);
                }
                if index >= 3 {
                    samples.push(elapsed);
                }
            }
            rows.push(json!({
                "nodes": count, "edges": count - 1, "groups": count.div_ceil(16),
                "operation": name, "selected_nodes": selected_count, "samples_ns": samples,
            }));
        }
    }
    println!(
        "P09_GRAPH_EDIT_MEASUREMENTS={}",
        json!({
            "fixture_version": 1,
            "manifest_dir": env!("CARGO_MANIFEST_DIR"),
            "warmups": 3,
            "samples_per_case": 31,
            "timed_region": "public transaction construction and commit, including rollback on rejection",
            "excluded": ["fixture creation", "whole-document clone", "whole-document drop", "assertions", "serialization"],
            "rows": rows,
        })
    );
}

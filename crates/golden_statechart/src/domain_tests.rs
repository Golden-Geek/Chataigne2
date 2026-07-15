use golden_graph::{GraphDomain, GraphEdge, GraphEnvelope, GraphTransaction, GraphTransactionError, PortRef};

use crate::{
    EnterPolicy, HistoryPolicy, StateUiLayout, Statechart, StatechartEdgeData, StatechartGraphDomain,
    StatechartGraphEnvelope, TransitionId,
};

fn representative_chart() -> Statechart {
    let mut chart = Statechart::new();
    let root = chart.root_region();
    let (parent, child_region) = chart
        .add_composite(root, "Parent", HistoryPolicy::Shallow, EnterPolicy::LastActiveChild)
        .unwrap();
    let first = chart.add_leaf(child_region, "First").unwrap();
    let second = chart.add_leaf(child_region, "Second").unwrap();
    let outside = chart.add_leaf(root, "Outside").unwrap();
    chart.set_initial(root, parent).unwrap();
    chart.set_initial(child_region, first).unwrap();
    chart.add_transition(first, outside, 10).unwrap();
    chart.add_transition(first, outside, 5).unwrap();
    chart.add_transition(outside, second, 0).unwrap();
    chart
        .set_state_ui_layout(
            parent,
            StateUiLayout {
                position: [2.0, 3.0],
                size: Some([24.0, 16.0]),
            },
        )
        .unwrap();
    chart
        .set_state_ui_layout(
            outside,
            StateUiLayout {
                position: [40.0, 3.0],
                size: None,
            },
        )
        .unwrap();
    chart.initialize().unwrap();
    chart
}

#[test]
fn statechart_is_the_common_graph_contract_without_a_conversion_adapter() {
    let chart = representative_chart();
    let document = chart.document();

    assert_eq!(document.id().as_uuid(), chart.id().as_uuid());
    assert_eq!(document.nodes().len(), chart.states().count());
    assert_eq!(document.edges().len(), chart.transitions().count());
    assert!(StatechartGraphDomain.validate_document(document).is_empty());
}

#[test]
fn statechart_domain_preserves_parallel_and_multi_incoming_transitions() {
    let chart = representative_chart();
    let outside = chart.states().find(|state| state.label == "Outside").unwrap();
    let source = chart.states().find(|state| state.label == "First").unwrap();
    let mut document = chart.document().clone();
    let target = PortRef::new(
        StatechartGraphDomain::node_id(outside.id),
        StatechartGraphDomain::incoming_port(),
    );
    assert_eq!(document.incoming_edges_for_port(target).count(), 2);

    let transition = TransitionId::new();
    let mut data = document.data().clone();
    let creation_order = data.next_transition_order;
    data.next_transition_order += 1;
    let mut transaction = GraphTransaction::for_document(&document);
    transaction.replace_graph_data(data);
    transaction.connect(GraphEdge {
        id: StatechartGraphDomain::edge_id(transition),
        from: PortRef::new(
            StatechartGraphDomain::node_id(source.id),
            StatechartGraphDomain::outgoing_port(),
        ),
        to: target,
        data: StatechartEdgeData {
            priority: 1,
            creation_order,
        },
    });

    transaction.commit(&mut document, &StatechartGraphDomain).unwrap();

    assert_eq!(document.incoming_edges_for_port(target).count(), 3);
    assert_eq!(Statechart::from_document(document).unwrap().transitions().count(), 4);
}

#[test]
fn inconsistent_region_edit_rolls_back_atomically() {
    let mut document = representative_chart().into_document();
    let before = document.clone();
    let mut data = document.data().clone();
    data.regions.get_mut(&data.root_region).unwrap().states.clear();
    let mut transaction = GraphTransaction::for_document(&document);
    transaction.replace_graph_data(data);

    let error = transaction.commit(&mut document, &StatechartGraphDomain).unwrap_err();

    assert!(matches!(error, GraphTransactionError::Validation(_)));
    assert_eq!(document, before);
}

#[test]
fn common_envelope_rebuilds_indexes_and_preserves_runtime_state() {
    let chart = representative_chart();
    let envelope = GraphEnvelope::from_document(
        StatechartGraphDomain.domain_id(),
        StatechartGraphDomain.schema_version(),
        chart.document(),
    );
    let encoded = serde_json::to_string(&envelope).unwrap();
    let decoded: StatechartGraphEnvelope = serde_json::from_str(&encoded).unwrap();
    let restored = decoded
        .into_document(
            StatechartGraphDomain.domain_id(),
            StatechartGraphDomain.schema_version(),
        )
        .unwrap();
    let restored = Statechart::from_document(restored).unwrap();

    assert_eq!(restored, chart);
    assert_eq!(restored.document().edges().len(), 3);
}

#[test]
fn serde_uses_the_versioned_graph_envelope() {
    let chart = representative_chart();
    let encoded = serde_json::to_string(&chart).unwrap();
    assert!(encoded.contains("golden.statechart"));
    assert_eq!(serde_json::from_str::<Statechart>(&encoded).unwrap(), chart);
}

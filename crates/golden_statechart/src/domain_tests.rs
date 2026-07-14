use golden_graph::{GraphDomain, GraphEdge, GraphEnvelope, GraphTransaction, GraphTransactionError, PortRef};

use crate::{
    EnterPolicy, HistoryPolicy, StateUiLayout, Statechart, StatechartEdgeData, StatechartGraphAdapter,
    StatechartGraphAdapterError, StatechartGraphDomain, StatechartGraphEnvelope, TransitionId,
};

fn representative_chart() -> Statechart {
    let mut chart = Statechart::new();
    let (parent, child_region) = chart
        .add_composite(
            chart.root_region,
            "Parent",
            HistoryPolicy::Shallow,
            EnterPolicy::LastActiveChild,
        )
        .unwrap();
    let first = chart.add_leaf(child_region, "First").unwrap();
    let second = chart.add_leaf(child_region, "Second").unwrap();
    let outside = chart.add_leaf(chart.root_region, "Outside").unwrap();
    chart.set_initial(chart.root_region, parent).unwrap();
    chart.set_initial(child_region, first).unwrap();
    chart.add_transition(first, outside, 10).unwrap();
    chart.add_transition(first, outside, 5).unwrap();
    chart.add_transition(outside, second, 0).unwrap();
    chart.states.get_mut(&parent).unwrap().ui_layout = StateUiLayout {
        position: [2.0, 3.0],
        size: Some([24.0, 16.0]),
    };
    chart.states.get_mut(&outside).unwrap().ui_layout.position = [40.0, 3.0];
    chart.initialize().unwrap();
    chart
}

#[test]
fn real_statechart_round_trips_through_common_graph_contract() {
    let chart = representative_chart();

    let document = StatechartGraphAdapter::to_document(&chart).unwrap();

    assert_eq!(document.id().as_uuid(), chart.id.as_uuid());
    assert_eq!(document.nodes().len(), chart.states.len());
    assert_eq!(document.edges().len(), chart.transitions.len());
    assert!(StatechartGraphDomain.validate_document(&document).is_empty());
    assert_eq!(StatechartGraphAdapter::to_legacy(&document).unwrap(), chart);
}

#[test]
fn statechart_domain_preserves_parallel_and_multi_incoming_transitions() {
    let chart = representative_chart();
    let outside = chart.states.values().find(|state| state.label == "Outside").unwrap();
    let mut document = StatechartGraphAdapter::to_document(&chart).unwrap();
    let target = PortRef::new(
        StatechartGraphDomain::node_id(outside.id),
        StatechartGraphDomain::incoming_port(),
    );
    assert_eq!(document.incoming_edges_for_port(target).count(), 2);

    let source = chart.states.values().find(|state| state.label == "First").unwrap();
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
    assert_eq!(
        StatechartGraphAdapter::to_legacy(&document).unwrap().transitions.len(),
        4
    );
}

#[test]
fn inconsistent_region_edit_rolls_back_atomically() {
    let mut document = StatechartGraphAdapter::to_document(&representative_chart()).unwrap();
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
fn common_envelope_rebuilds_multi_edge_indexes_and_preserves_runtime_state() {
    let chart = representative_chart();
    let document = StatechartGraphAdapter::to_document(&chart).unwrap();
    let envelope = GraphEnvelope::from_document(
        StatechartGraphDomain.domain_id(),
        StatechartGraphDomain.schema_version(),
        &document,
    );
    let encoded = serde_json::to_string(&envelope).unwrap();
    let decoded: StatechartGraphEnvelope = serde_json::from_str(&encoded).unwrap();

    let restored = decoded
        .into_document(
            StatechartGraphDomain.domain_id(),
            StatechartGraphDomain.schema_version(),
        )
        .unwrap();

    assert_eq!(StatechartGraphAdapter::to_legacy(&restored).unwrap(), chart);
    assert_eq!(restored.edges().len(), 3);
}

#[test]
fn adapter_rejects_duplicate_transition_ids_instead_of_dropping_data() {
    let mut chart = representative_chart();
    chart.transitions.push(chart.transitions[0].clone());

    let error = StatechartGraphAdapter::to_document(&chart).unwrap_err();

    assert!(matches!(error, StatechartGraphAdapterError::DuplicateTransition(_)));
}

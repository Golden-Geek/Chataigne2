use super::*;
use golden_graph::GraphDocumentData;

#[test]
fn statechart_consumes_batched_layout_without_reordering_states_or_changing_execution() {
    let chart = representative_chart();
    let states = chart.states().map(|state| state.id).collect::<Vec<_>>();
    let selected = [states[0], *states.last().unwrap()];
    let active = chart.active().clone();
    let edges = chart.document().edges().map(|edge| edge.id).collect::<Vec<_>>();
    let mut document = chart.into_document();
    let before = document.revision();
    let mut transaction = GraphTransaction::for_document(&document);
    for (index, state) in selected.iter().enumerate() {
        let id = StatechartGraphDomain::node_id(*state);
        let mut layout = document.presentation().nodes[&id].clone();
        layout.position = [100.0 + index as f64, 50.0];
        transaction.set_node_presentation(id, Some(layout));
    }
    let moved = transaction.commit(&mut document, &StatechartGraphDomain).unwrap();
    assert_eq!(moved.delta.to.topology, before.topology);
    assert_eq!(moved.delta.to.payload, before.payload);
    assert_eq!(moved.delta.to.presentation, before.presentation + 1);
    let loaded = GraphDocumentData::from_document(&document).into_document().unwrap();
    let chart = Statechart::from_document(loaded).unwrap();
    assert_eq!(chart.states().map(|state| state.id).collect::<Vec<_>>(), states);
    assert_eq!(chart.document().edges().map(|edge| edge.id).collect::<Vec<_>>(), edges);
    assert_eq!(chart.active(), &active);
    for (index, state) in selected.iter().enumerate() {
        assert_eq!(
            chart.state(*state).unwrap().ui_layout.position,
            [100.0 + index as f64, 50.0]
        );
    }
}

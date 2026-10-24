use super::*;
use golden_graph::GraphDocumentData;

#[test]
fn selection_layout_delete_and_restore_preserve_alchemist_semantics() {
    let domain = AlchemistGraphDomain::with_primitives();
    let (mut document, _, _) = representative_document();
    let nodes = document
        .nodes()
        .map(|node| (node.id, node.data.clone()))
        .collect::<Vec<_>>();
    let edges = document.edges().map(|edge| edge.id).collect::<Vec<_>>();
    let before = document.revision();
    let mut transaction = GraphTransaction::for_document(&document);
    for (index, (id, _)) in nodes.iter().enumerate() {
        let mut layout = document.presentation().nodes[id].clone();
        layout.position = [100.0 + index as f64, 50.0];
        transaction.set_node_presentation(*id, Some(layout));
    }
    let moved = transaction.commit(&mut document, &domain).unwrap();
    assert_eq!(moved.delta.to.topology, before.topology);
    assert_eq!(moved.delta.to.payload, before.payload);
    assert_eq!(moved.delta.to.presentation, before.presentation + 1);
    assert_eq!(
        document.nodes().map(|node| node.id).collect::<Vec<_>>(),
        nodes.iter().map(|(id, _)| *id).collect::<Vec<_>>()
    );
    assert_eq!(document.edges().map(|edge| edge.id).collect::<Vec<_>>(), edges);
    for (index, (id, data)) in nodes.iter().enumerate() {
        assert_eq!(&document.node(*id).unwrap().data, data);
        assert_eq!(document.presentation().nodes[id].position, [100.0 + index as f64, 50.0]);
    }
    assert!(domain.validate_document(&document).is_empty());
    let restored = GraphDocumentData::from_document(&document).into_document().unwrap();
    assert_eq!(restored, document);
    let mut transaction = GraphTransaction::for_document(&document);
    for (id, _) in &nodes {
        transaction.remove_node(*id);
    }
    let deleted = transaction.commit(&mut document, &domain).unwrap();
    assert_eq!(deleted.delta.changes.removed_nodes.len(), 2);
    assert_eq!(deleted.delta.changes.removed_edges.len(), 1);
    assert_eq!(document.nodes().len(), 0);
    assert_eq!(document.edges().len(), 0);
    assert!(domain.validate_document(&document).is_empty());
    assert!(domain.validate_document(&restored).is_empty());
    assert_eq!(
        restored.nodes().map(|node| node.id).collect::<Vec<_>>(),
        nodes.iter().map(|(id, _)| *id).collect::<Vec<_>>()
    );
}

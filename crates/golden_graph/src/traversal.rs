use std::collections::{BTreeSet, HashMap, HashSet};

use crate::{GraphDocument, GraphNodeId};

/// Returns a deterministic topological order or the stable set of nodes in cycles.
pub fn stable_topological_order<G, N, E>(graph: &GraphDocument<G, N, E>) -> Result<Vec<GraphNodeId>, Vec<GraphNodeId>> {
    let mut indegree = graph.nodes().map(|node| (node.id, 0usize)).collect::<HashMap<_, _>>();
    for edge in graph.edges() {
        *indegree.get_mut(&edge.to.node).expect("edge target is indexed") += 1;
    }

    let order_index = graph
        .nodes()
        .enumerate()
        .map(|(index, node)| (node.id, index))
        .collect::<HashMap<_, _>>();
    let mut ready = indegree
        .iter()
        .filter_map(|(node, degree)| (*degree == 0).then_some((order_index[node], *node)))
        .collect::<BTreeSet<_>>();
    let mut ordered = Vec::with_capacity(indegree.len());

    while let Some((index, node)) = ready.pop_first() {
        let _ = index;
        ordered.push(node);
        for edge_id in graph.outgoing_edges(node) {
            let target = graph.edge(edge_id).expect("topology edge is indexed").to.node;
            let degree = indegree.get_mut(&target).expect("edge target is indexed");
            *degree -= 1;
            if *degree == 0 {
                ready.insert((order_index[&target], target));
            }
        }
    }

    if ordered.len() == indegree.len() {
        Ok(ordered)
    } else {
        Err(graph
            .nodes()
            .filter(|node| indegree[&node.id] > 0)
            .map(|node| node.id)
            .collect())
    }
}

/// Returns strongly connected components in deterministic document order.
pub fn strongly_connected_components<G, N, E>(graph: &GraphDocument<G, N, E>) -> Vec<Vec<GraphNodeId>> {
    let node_order = graph
        .nodes()
        .enumerate()
        .map(|(index, node)| (node.id, index))
        .collect::<HashMap<_, _>>();
    let mut visited = HashSet::new();
    let mut finish = Vec::with_capacity(node_order.len());

    for root in graph.nodes().map(|node| node.id) {
        if visited.contains(&root) {
            continue;
        }
        let mut stack = vec![(root, false)];
        while let Some((node, expanded)) = stack.pop() {
            if expanded {
                finish.push(node);
                continue;
            }
            if !visited.insert(node) {
                continue;
            }
            stack.push((node, true));
            let mut neighbors = graph
                .outgoing_edges(node)
                .map(|edge| graph.edge(edge).expect("topology edge is indexed").to.node)
                .collect::<Vec<_>>();
            neighbors.sort_unstable_by_key(|neighbor| node_order[neighbor]);
            for neighbor in neighbors.into_iter().rev() {
                if !visited.contains(&neighbor) {
                    stack.push((neighbor, false));
                }
            }
        }
    }

    visited.clear();
    let mut components = Vec::new();
    for root in finish.into_iter().rev() {
        if !visited.insert(root) {
            continue;
        }
        let mut component = Vec::new();
        let mut stack = vec![root];
        while let Some(node) = stack.pop() {
            component.push(node);
            let mut neighbors = graph
                .incoming_edges(node)
                .map(|edge| graph.edge(edge).expect("topology edge is indexed").from.node)
                .collect::<Vec<_>>();
            neighbors.sort_unstable_by_key(|neighbor| node_order[neighbor]);
            for neighbor in neighbors.into_iter().rev() {
                if visited.insert(neighbor) {
                    stack.push(neighbor);
                }
            }
        }
        component.sort_unstable_by_key(|node| node_order[node]);
        components.push(component);
    }
    components.sort_by_key(|component| node_order[&component[0]]);
    components
}

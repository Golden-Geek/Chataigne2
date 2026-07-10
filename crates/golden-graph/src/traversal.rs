use std::collections::{BTreeMap, BTreeSet};

use crate::{GraphDocument, GraphDomain, GraphNodeId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphCycle {
    pub remaining: Vec<GraphNodeId>,
}

pub fn stable_topological_order<D: GraphDomain>(graph: &GraphDocument<D>) -> Result<Vec<GraphNodeId>, GraphCycle> {
    let mut indegree = graph
        .nodes()
        .map(|node| (node.id, graph.incoming_edges(node.id).count()))
        .collect::<BTreeMap<_, _>>();
    let mut ready = indegree
        .iter()
        .filter_map(|(node, degree)| (*degree == 0).then_some(*node))
        .collect::<BTreeSet<_>>();
    let mut order = Vec::with_capacity(indegree.len());

    while let Some(node) = ready.pop_first() {
        order.push(node);
        for edge in graph.outgoing_edges(node) {
            let degree = indegree
                .get_mut(&edge.to.node)
                .expect("edge destination must have an indegree");
            *degree -= 1;
            if *degree == 0 {
                ready.insert(edge.to.node);
            }
        }
    }

    if order.len() == indegree.len() {
        Ok(order)
    } else {
        Err(GraphCycle {
            remaining: indegree
                .into_iter()
                .filter_map(|(node, degree)| (degree > 0).then_some(node))
                .collect(),
        })
    }
}

pub fn strongly_connected_components<D: GraphDomain>(graph: &GraphDocument<D>) -> Vec<Vec<GraphNodeId>> {
    let mut visited = BTreeSet::new();
    let mut finish = Vec::with_capacity(graph.nodes().len());

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
            let mut targets = graph.outgoing_edges(node).map(|edge| edge.to.node).collect::<Vec<_>>();
            targets.sort_unstable_by(|left, right| right.cmp(left));
            stack.extend(targets.into_iter().map(|target| (target, false)));
        }
    }

    let mut assigned = BTreeSet::new();
    let mut components = Vec::new();
    for root in finish.into_iter().rev() {
        if !assigned.insert(root) {
            continue;
        }
        let mut component = Vec::new();
        let mut stack = vec![root];
        while let Some(node) = stack.pop() {
            component.push(node);
            let mut sources = graph
                .incoming_edges(node)
                .map(|edge| edge.from.node)
                .collect::<Vec<_>>();
            sources.sort_unstable_by(|left, right| right.cmp(left));
            for source in sources {
                if assigned.insert(source) {
                    stack.push(source);
                }
            }
        }
        component.sort_unstable();
        components.push(component);
    }
    components.sort_by_key(|component| component.first().copied());
    components
}

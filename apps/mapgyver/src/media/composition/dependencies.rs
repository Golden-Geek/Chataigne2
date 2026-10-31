use crate::{
    app::{AppEngine, AppNode},
    media::protocol::{ChannelId, ChannelReference, ChannelSource, MediaId},
};
use golden_core::node::Node;
use std::collections::{HashMap, HashSet};

fn graph(engine: &AppEngine) -> Result<HashMap<ChannelReference, Vec<ChannelReference>>, String> {
    let mut result = HashMap::new();
    for (id, node) in engine.nodes.iter() {
        let AppNode::MediaChannel(node) = node else {
            continue;
        };
        if !matches!(node.definition().source, ChannelSource::Composition { .. }) {
            continue;
        }
        let Some(AppNode::Media(media)) = node.node_data().parent.and_then(|id| engine.nodes.get(id)) else {
            return Err("Composition owner is missing".into());
        };
        let reference = ChannelReference {
            media: MediaId(media.node_data().meta.uuid),
            channel: ChannelId(node.node_data().meta.uuid),
        };
        let mut sources = Vec::new();
        for instance in super::authoring::instances(engine, id)? {
            let AppNode::CompositionInstance(node) = engine.nodes.get(instance).unwrap() else {
                unreachable!()
            };
            node.definition().validate()?;
            if let Some(source) = node.definition().source {
                sources.push(source);
            }
        }
        result.insert(reference, sources);
    }
    Ok(result)
}

/// Existing graphs are acyclic; introducing owner -> sources must not reach owner again.
pub(super) fn reject_cycle(
    engine: &AppEngine,
    owner: ChannelReference,
    sources: &[ChannelReference],
) -> Result<(), String> {
    let graph = graph(engine)?;
    let mut pending = sources.to_vec();
    let mut seen = HashSet::new();
    while let Some(source) = pending.pop() {
        if source == owner {
            return Err("Composition dependency cycle".into());
        }
        if seen.insert(source)
            && let Some(children) = graph.get(&source)
        {
            pending.extend(children.iter().copied());
        }
    }
    Ok(())
}

/// Iterative topological validation also admits unresolved authored references as missing leaves.
pub(crate) fn validate_project(engine: &AppEngine) -> Result<(), String> {
    let graph = graph(engine)?;
    let mut incoming = graph.keys().map(|key| (*key, 0usize)).collect::<HashMap<_, _>>();
    for sources in graph.values() {
        for source in sources {
            if let Some(count) = incoming.get_mut(source) {
                *count += 1;
            }
        }
    }
    let mut ready = incoming
        .iter()
        .filter_map(|(node, count)| (*count == 0).then_some(*node))
        .collect::<Vec<_>>();
    let mut visited = 0;
    while let Some(node) = ready.pop() {
        visited += 1;
        for source in &graph[&node] {
            if let Some(count) = incoming.get_mut(source) {
                *count -= 1;
                if *count == 0 {
                    ready.push(*source);
                }
            }
        }
    }
    if visited == graph.len() {
        Ok(())
    } else {
        Err("Composition dependency cycle".into())
    }
}

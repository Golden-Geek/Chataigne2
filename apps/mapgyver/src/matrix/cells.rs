use super::{
    authoring::{MAX_BATCH, mutate},
    document::Document,
    node::ElementNode,
    protocol::*,
};
use crate::{
    app::{AppEngine, AppNode},
    media::protocol::ChannelReference,
};
use golden_core::edit::{Edit, NodeTree, UserItemTreeInsertion};
use golden_core::node::Node;
use std::collections::{HashMap, HashSet};

pub(super) fn assign(
    engine: &mut AppEngine,
    document: &Document,
    assignments: Vec<CellAssignment>,
) -> Result<(), String> {
    if assignments.is_empty() || assignments.len() > MAX_BATCH {
        return Err("assign 1..4096 Matrix cells per atomic batch".into());
    }
    let mut seen = HashSet::new();
    let mut additions = 0;
    let mut changes = HashMap::new();
    for assignment in assignments {
        document.item(MatrixItemId::Layer { id: assignment.layer })?;
        document.column(assignment.column)?;
        if !seen.insert((assignment.layer, assignment.column)) {
            return Err("duplicate Matrix cell address in one batch".into());
        }
        if let Some(source) = assignment.source {
            crate::media::channel(engine, source)?;
        }
        if !document.cells.contains_key(&(assignment.layer, assignment.column)) {
            additions += 1;
        }
        changes.insert((assignment.layer, assignment.column), assignment.source);
    }
    document.admit(additions)?;
    let affected = changes.keys().map(|(layer, _)| *layer).collect::<HashSet<_>>();
    let mut edges = HashMap::<LayerId, Vec<ChannelReference>>::new();
    for cell in &document.snapshot.cells {
        if affected.contains(&cell.layer)
            && !changes.contains_key(&(cell.layer, cell.definition.column))
            && let Some(source) = cell.definition.source
        {
            edges.entry(cell.layer).or_default().push(source);
        }
    }
    for ((layer, _), source) in &changes {
        if let Some(source) = source {
            edges.entry(*layer).or_default().push(*source);
        }
    }
    let replacements = affected
        .into_iter()
        .map(|layer| {
            (
                document.channel(MatrixItemId::Layer { id: layer }),
                edges.remove(&layer).unwrap_or_default(),
            )
        })
        .collect();
    crate::media::dependencies::replace_sources(engine, replacements)?;
    let mut insertions = Vec::with_capacity(additions);
    for ((layer, column), source) in changes {
        let definition = ElementDefinition::Cell {
            definition: CellDefinition { column, source },
        };
        if let Some(id) = document.cells.get(&(layer, column)) {
            let Some(AppNode::MatrixElement(current)) = engine.nodes.get(*id) else {
                unreachable!()
            };
            if current.definition() != &definition {
                mutate(engine, *id, definition);
            }
        } else {
            let parent = document.item(MatrixItemId::Layer { id: layer })?;
            let label = format!(
                "Cell: {}",
                engine
                    .nodes
                    .get(document.column(column)?)
                    .unwrap()
                    .node_data()
                    .meta
                    .label
            );
            insertions.push(UserItemTreeInsertion::new(
                parent,
                NodeTree::new(ElementNode::new(label, definition)),
            ));
        }
    }
    if !insertions.is_empty() {
        engine.edits.push(Edit::AddUserItemTrees { items: insertions });
    }
    Ok(())
}

use super::{
    authoring::{descendants, export, item_id},
    document::{Document, children},
    node::ElementNode,
    protocol::*,
};
use crate::{
    app::{AppEngine, AppNode, labels},
    media::protocol::{ChannelReference, MediaEditResult},
};
use golden_core::{
    edit::{Edit, NodeTree, UserItemTreeInsertion},
    node::{Node, NodeId},
};
use std::collections::HashMap;

pub(super) fn column(engine: &mut AppEngine, document: &Document, column: ColumnId) -> Result<MediaEditResult, String> {
    document.column(column)?;
    let source = &document
        .snapshot
        .columns
        .iter()
        .find(|entry| entry.id == column)
        .unwrap();
    let label = labels::unique(
        engine,
        document.root,
        Some(format!("{} copy", source.label)),
        "Column",
        &[],
    )?;
    let node = ElementNode::new(
        label,
        ElementDefinition::Column {
            definition: source.definition.clone(),
        },
    );
    let created = ColumnId(node.node_data().meta.uuid);
    let count = document
        .snapshot
        .cells
        .iter()
        .filter(|cell| cell.definition.column == column)
        .count();
    document.admit(count + 1)?;
    let mut items = vec![UserItemTreeInsertion::new(document.root, NodeTree::new(node))];
    for cell in &document.snapshot.cells {
        if cell.definition.column == column {
            let parent = document.item(MatrixItemId::Layer { id: cell.layer })?;
            let definition = ElementDefinition::Cell {
                definition: CellDefinition {
                    column: created,
                    source: cell.definition.source,
                },
            };
            items.push(UserItemTreeInsertion::new(
                parent,
                NodeTree::new(ElementNode::new("Cell copy".into(), definition)),
            ));
        }
    }
    engine.edits.push(Edit::AddUserItemTrees { items });
    Ok(MediaEditResult::MatrixCreated {
        items: vec![],
        columns: vec![created],
    })
}

pub(super) fn item(engine: &mut AppEngine, document: &Document, item: MatrixItemId) -> Result<MediaEditResult, String> {
    let root = document.item(item)?;
    let all = descendants(engine, root)?;
    document.admit(all.len())?;
    let parent = engine.nodes.get(root).unwrap().node_data().parent.unwrap();
    let mut pending = vec![root];
    let mut order = Vec::with_capacity(all.len());
    while let Some(id) = pending.pop() {
        order.push(id);
        pending.extend(children(engine, id)?.into_iter().rev());
    }
    let mut nodes = HashMap::new();
    let mut remap = HashMap::<ChannelReference, ChannelReference>::new();
    let mut exports = Vec::new();
    let mut created_root = None;
    for id in &order {
        let Some(AppNode::MatrixElement(original)) = engine.nodes.get(*id) else {
            unreachable!()
        };
        let label = if *id == root {
            labels::unique(
                engine,
                parent,
                Some(format!("{} copy", original.node_data().meta.label)),
                "Copy",
                &[],
            )?
        } else {
            original.node_data().meta.label.clone()
        };
        let node = ElementNode::new(label.clone(), original.definition().clone());
        if matches!(
            original.definition(),
            ElementDefinition::Layer { .. } | ElementDefinition::Group { .. }
        ) {
            let old = item_id(original);
            let new = item_id(&node);
            if *id == root {
                created_root = Some(new);
            }
            let (channel, reference) = export(document.snapshot.media, new, format!("{label} output"));
            remap.insert(document.channel(old), reference);
            exports.push(UserItemTreeInsertion::new(document.media, channel));
        }
        nodes.insert(*id, node);
    }
    let mut trees = HashMap::<NodeId, NodeTree>::new();
    for id in order.into_iter().rev() {
        let mut node = nodes.remove(&id).unwrap();
        if let ElementDefinition::Cell { definition } = node.definition().clone() {
            let source = definition
                .source
                .map(|source| remap.get(&source).copied().unwrap_or(source));
            node.set_definition(ElementDefinition::Cell {
                definition: CellDefinition { source, ..definition },
            })?;
        }
        let mut tree = NodeTree::new(node).as_user_item();
        for child in children(engine, id)? {
            tree = tree.with_child(trees.remove(&child).unwrap());
        }
        trees.insert(id, tree);
    }
    let mut items = vec![UserItemTreeInsertion::new(parent, trees.remove(&root).unwrap())];
    items.extend(exports);
    engine.edits.push(Edit::AddUserItemTrees { items });
    Ok(MediaEditResult::MatrixCreated {
        items: vec![created_root.unwrap()],
        columns: vec![],
    })
}

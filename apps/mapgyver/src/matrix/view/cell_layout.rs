use super::*;

fn cell(engine: &AppEngine, media: MediaId, layer: LayerId, node: NodeId) -> Result<MatrixLayoutCell, String> {
    let view = cell_snapshot(engine, layer, node)?;
    let extent = view
        .cell
        .definition
        .source
        .and_then(|source| crate::media::resolution::channel_extent(engine, source).ok());
    Ok(MatrixLayoutCell {
        label: engine
            .nodes
            .get(node)
            .ok_or("Cell changed")?
            .node_data()
            .meta
            .label
            .clone(),
        preview: crate::preview::protocol::PreviewSource::MatrixCell {
            media,
            cell: view.cell.id,
        },
        view,
        extent,
    })
}

pub(crate) fn inspect(engine: &AppEngine, query: MatrixCellLayoutQuery) -> Result<MatrixCellLayoutPage, String> {
    if query.count == 0 || query.count > 128 {
        return Err("Cell layout admits 1..128 definitions per page".into());
    }
    let mut page = MatrixCellLayoutPage {
        at: engine.time,
        parameters: Vec::new(),
        media: query.media,
        layer: query.layer,
        row: None,
        context: None,
        total: 0,
        start: query.start,
        cells: vec![],
        selected_id: query.selected,
        selected: None,
    };
    if engine.node_id_by_uuid(query.media.0).is_none() {
        return Ok(page);
    }
    let index = index(engine, query.media)?;
    let target = MatrixItemId::Layer { id: query.layer };
    let Some(&position) = index.row_positions.get(&target) else {
        return Ok(page);
    };
    let row = &index.rows[position];
    let Some(AppNode::MatrixElement(layer)) = engine.nodes.get(row.node) else {
        return Err("Layer changed".into());
    };
    page.parameters.push(layer.geometry_bindings(engine, query.layer.0));
    page.row = Some(row_snapshot(engine, row)?);
    let root = super::super::playback::root(engine, query.media)?;
    page.context = Some(crate::media::channel(
        engine,
        crate::media::protocol::ChannelReference {
            media: query.media,
            channel: crate::media::protocol::ChannelId(
                engine.nodes.get(root).ok_or("Matrix changed")?.node_data().meta.uuid,
            ),
        },
    )?);
    let cells = index.layer_cells.get(&query.layer).map_or(&[][..], Vec::as_slice);
    page.total = cells.len() as u32;
    page.start = query.start.min(page.total);
    for &node in cells.iter().skip(page.start as usize).take(query.count as usize) {
        page.cells.push(cell(engine, query.media, query.layer, node)?);
    }
    let mut next = query.node.and_then(|uuid| engine.node_id_by_uuid(uuid));
    for _ in 0..64 {
        let Some(node) = next.and_then(|id| engine.nodes.get(id)) else {
            break;
        };
        if let AppNode::MatrixCell(_) = node {
            if node.node_data().parent == Some(row.node) {
                page.selected_id = Some(CellId(node.node_data().meta.uuid));
            }
            break;
        }
        if matches!(node, AppNode::MatrixElement(_) | AppNode::Media(_)) {
            break;
        }
        next = node.node_data().parent;
    }
    if let Some(id) = page.selected_id.and_then(|cell| engine.node_id_by_uuid(cell.0))
        && engine
            .nodes
            .get(id)
            .is_some_and(|node| matches!(node, AppNode::MatrixCell(_)) && node.node_data().parent == Some(row.node))
    {
        page.selected = Some(cell(engine, query.media, query.layer, id)?);
    }
    for cell in page.cells.iter().chain(page.selected.iter()) {
        let id = cell.view.cell.id.0;
        if page.parameters.iter().any(|binding| binding.item == id) {
            continue;
        }
        let Some(AppNode::MatrixCell(node)) = engine.node_id_by_uuid(id).and_then(|id| engine.nodes.get(id)) else {
            return Err("Cell changed".into());
        };
        page.parameters.push(node.geometry_bindings(engine, id));
    }
    Ok(page)
}

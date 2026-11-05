use super::{document::Document, protocol::*};

/// Resolve presentation targets against the complete committed order, not the visible window.
pub(super) fn resolve(document: &Document, edit: MatrixEdit) -> Result<Option<MatrixEdit>, String> {
    let resolved = match edit {
        MatrixEdit::DropItem { item, target } => {
            document.item(item)?;
            let (parent, after) = match target {
                MatrixItemDropTarget::Root => (None, last_child(document, None, item)),
                MatrixItemDropTarget::Inside { group } => {
                    document.item(MatrixItemId::Group { id: group })?;
                    (Some(group), last_child(document, Some(group), item))
                }
                MatrixItemDropTarget::Before { item: target } => beside(document, item, target, true)?,
                MatrixItemDropTarget::After { item: target } => beside(document, item, target, false)?,
            };
            MatrixEdit::MoveItem { item, parent, after }
        }
        MatrixEdit::DropColumn { column, target, before } => {
            document.column(column)?;
            document.column(target)?;
            if column == target {
                return Err("a Column cannot be dropped on itself".into());
            }
            let parent = document
                .snapshot
                .columns
                .iter()
                .find(|entry| entry.id == target)
                .unwrap()
                .parent;
            let after = if before {
                super::column_groups::siblings(&document.snapshot, parent)
                    .into_iter()
                    .filter(|entry| *entry != ColumnItemId::Column { id: column })
                    .take_while(|entry| *entry != ColumnItemId::Column { id: target })
                    .last()
            } else {
                Some(ColumnItemId::Column { id: target })
            };
            MatrixEdit::MoveColumnItem {
                item: ColumnItemId::Column { id: column },
                parent,
                after,
            }
        }
        edit @ MatrixEdit::MoveColumnItem { .. } => edit,
        edit => return Ok(Some(edit)),
    };
    let unchanged = match &resolved {
        MatrixEdit::MoveItem { item, parent, after } => {
            let row = document.snapshot.items.iter().find(|row| row.id == *item).unwrap();
            row.parent == *parent
                && document
                    .snapshot
                    .items
                    .iter()
                    .filter(|entry| entry.parent == *parent)
                    .take_while(|entry| entry.id != *item)
                    .last()
                    .map(|entry| entry.id)
                    == *after
        }
        MatrixEdit::MoveColumnItem { item, parent, after } => {
            document.column_item(*item)?;
            let current_parent = match item {
                ColumnItemId::Column { id } => document.snapshot.columns.iter().find(|c| c.id == *id).unwrap().parent,
                ColumnItemId::Group { id } => {
                    document
                        .snapshot
                        .column_groups
                        .iter()
                        .find(|g| g.id == *id)
                        .unwrap()
                        .parent
                }
            };
            current_parent == *parent
                && super::column_groups::siblings(&document.snapshot, *parent)
                    .into_iter()
                    .take_while(|entry| entry != item)
                    .last()
                    == *after
        }
        _ => unreachable!(),
    };
    Ok((!unchanged).then_some(resolved))
}

fn beside(
    document: &Document,
    moving: MatrixItemId,
    target: MatrixItemId,
    before: bool,
) -> Result<(Option<GroupId>, Option<MatrixItemId>), String> {
    document.item(target)?;
    if target == moving {
        return Err("a Matrix row cannot be dropped on itself".into());
    }
    let row = document.snapshot.items.iter().find(|row| row.id == target).unwrap();
    let after = if before {
        document
            .snapshot
            .items
            .iter()
            .filter(|entry| entry.parent == row.parent && entry.id != moving)
            .take_while(|entry| entry.id != target)
            .last()
            .map(|entry| entry.id)
    } else {
        Some(target)
    };
    Ok((row.parent, after))
}

fn last_child(document: &Document, parent: Option<GroupId>, moving: MatrixItemId) -> Option<MatrixItemId> {
    document
        .snapshot
        .items
        .iter()
        .rfind(|row| row.parent == parent && row.id != moving)
        .map(|row| row.id)
}

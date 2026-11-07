//! One public-intent scene shared by process final-use and native transfer pixel acceptance.
use super::*;

pub struct Transfers {
    pub source: CellId,
    pub copy: CellId,
    pub reference: CellId,
    pub source_output: ChannelReference,
    pub source_row: LayerId,
    pub source_column: ColumnId,
    row: LayerId,
    copy_column: ColumnId,
    reference_column: ColumnId,
}

impl Transfers {
    pub fn prepare(scene: &mut Scene) -> Self {
        let MediaEditResult::Matrix { snapshot } = scene.matrix(MatrixEdit::Inspect) else {
            panic!()
        };
        let source = snapshot.cells.iter().find(|cell| cell.id == scene.cells[0]).unwrap();
        // The scoped-control fixture starts with catalog aliases. This acceptance needs
        // a real owned producer so Copy can prove independence from live source edits.
        scene.matrix(MatrixEdit::SetCells {
            assignments: vec![CellAssignment {
                layer: source.layer,
                column: source.definition.column,
                source: None,
            }],
        });
        scene.matrix(MatrixEdit::CreateCellMedia {
            layer: source.layer,
            column: source.definition.column,
            media_kind: MediaKind::SolidColor,
        });
        let MediaEditResult::Matrix { snapshot: owned } = scene.matrix(MatrixEdit::Inspect) else {
            panic!()
        };
        let source = owned.cells.iter().find(|cell| cell.id == scene.cells[0]).unwrap();
        let source_output = source.definition.source.unwrap();
        edit(
            &mut scene.engine,
            MediaEdit::ConfigureChannel {
                reference: source_output,
                source: ChannelSource::SolidColor {
                    rgba: [1.0, 0.0, 0.0, 1.0],
                },
            },
        );
        edit(
            &mut scene.engine,
            MediaEdit::ConfigureMediaResolution {
                media: source_output.media,
                size: SizePolicy::Fixed {
                    extent: Extent2D::new(16, 24).unwrap(),
                },
            },
        );
        scene.play();
        let row = snapshot
            .items
            .iter()
            .find_map(|item| match item.id {
                MatrixItemId::Layer { id } if !scene.rows.contains(&id) => Some(id),
                _ => None,
            })
            .unwrap();
        let duplicate = |scene: &mut Scene, column, action| {
            let MediaEditResult::MatrixCellPrepared { cell, .. } = scene.matrix(MatrixEdit::TransferCell {
                cell: source.id,
                layer: row,
                column,
                action,
            }) else {
                panic!()
            };
            cell
        };
        let copy_column = scene.columns[1];
        let reference_column = scene.columns[2];
        let copy = duplicate(scene, copy_column, CellTransferAction::Copy);
        let reference = duplicate(scene, reference_column, CellTransferAction::Reference);
        Self {
            source: source.id,
            copy,
            reference,
            source_output,
            source_row: source.layer,
            source_column: source.definition.column,
            row,
            copy_column,
            reference_column,
        }
    }

    pub fn launch(&self, scene: &mut Scene, action: CellTransferAction) {
        scene.matrix(MatrixEdit::LaunchCell {
            layer: self.row,
            column: match action {
                CellTransferAction::Copy => self.copy_column,
                CellTransferAction::Reference => self.reference_column,
                _ => panic!("the lifetime scene launches Copy or Reference"),
            },
            mode: LaunchMode::Cut,
        });
    }

    pub fn clear_source(&self, scene: &mut Scene) {
        scene.matrix(MatrixEdit::SetCells {
            assignments: vec![CellAssignment {
                layer: self.source_row,
                column: self.source_column,
                source: None,
            }],
        });
    }

    pub fn remove_source_owner(&self, scene: &mut Scene) {
        scene.matrix(MatrixEdit::RemoveItem {
            item: MatrixItemId::Layer { id: self.source_row },
        });
    }

    pub fn color_source(&self, scene: &mut Scene, rgba: [f32; 4]) {
        edit(
            &mut scene.engine,
            MediaEdit::ConfigureChannel {
                reference: self.source_output,
                source: ChannelSource::SolidColor { rgba },
            },
        );
    }
}

//! Shared public-contract scene for scoped control software, process and native pixel checks.
use golden_core::{
    app::{create_new_project_engine, prepare_engine_for_runtime, to_sparse_project_json_pretty},
    application::{apply_project_intent, service_script_app_operations},
    edit::Edit,
    node::{Node, NodeId, NodeUuid},
    parameter::{ParamValue, ParameterEventBehaviour},
    process_ctx::ExecutionPhase,
    script::ScriptPropertyWrite,
    ui_sync::UiEditIntent,
};
use golden_video::Extent2D;
use mapgyver::{
    app::{AppEngine, AppNode},
    applications::protocol::ApplicationDefinition,
    matrix::protocol::*,
    media::protocol::*,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    Cell,
    Layer,
    LayerGroup,
    Column,
    ColumnGroup,
    All,
}
pub const SCOPES: [Scope; 6] = [
    Scope::Cell,
    Scope::Layer,
    Scope::LayerGroup,
    Scope::Column,
    Scope::ColumnGroup,
    Scope::All,
];
#[derive(Clone, Copy, Debug)]
pub enum Caller {
    Transport,
    Script,
}
pub struct Scene {
    pub engine: AppEngine,
    pub root: ChannelReference,
    rows: [LayerId; 2],
    columns: [ColumnId; 3],
    cells: [CellId; 2],
    layer_group: GroupId,
    column_group: ColumnGroupId,
}
fn edit(engine: &mut AppEngine, operation: MediaEdit) -> MediaEditResult {
    let ack = apply_project_intent(
        engine,
        UiEditIntent::InvokeAppOperation {
            namespace: MEDIA_NAMESPACE.into(),
            operation: MEDIA_OPERATION.into(),
            payload: serde_json::to_value(operation).unwrap(),
        },
    );
    assert!(ack.success, "{ack:?}");
    service_script_app_operations(engine).unwrap();
    serde_json::from_value(ack.result.unwrap().value).unwrap()
}
impl Scene {
    pub fn new(application: Option<ApplicationDefinition>) -> Self {
        let mut engine = create_new_project_engine::<AppNode>().unwrap();
        engine.apply_edits().unwrap();
        prepare_engine_for_runtime(&mut engine).unwrap();
        let MediaEditResult::Created { reference: root } = edit(
            &mut engine,
            MediaEdit::CreateMedia {
                media_kind: MediaKind::Matrix,
                label: Some("Scoped controls".into()),
            },
        ) else {
            panic!()
        };
        edit(
            &mut engine,
            MediaEdit::ConfigureMediaResolution {
                media: root.media,
                size: SizePolicy::Fixed {
                    extent: Extent2D::new(32, 24).unwrap(),
                },
            },
        );
        let matrix = |engine: &mut AppEngine, operation| {
            edit(
                engine,
                MediaEdit::Matrix {
                    media: root.media,
                    edit: operation,
                },
            )
        };
        let MediaEditResult::Matrix { snapshot } = matrix(&mut engine, MatrixEdit::Inspect) else {
            panic!()
        };
        let rows: Vec<_> = snapshot
            .items
            .iter()
            .map(|row| match row.id {
                MatrixItemId::Layer { id } => id,
                _ => panic!(),
            })
            .collect();
        let columns = std::array::from_fn(|i| snapshot.columns[i].id);
        for row in &rows {
            matrix(
                &mut engine,
                MatrixEdit::ConfigureLayer {
                    layer: *row,
                    settings: LayerSettings {
                        size: Extent2D::new(32, 24).unwrap(),
                        ..snapshot
                            .items
                            .iter()
                            .find(|item| item.id == MatrixItemId::Layer { id: *row })
                            .unwrap()
                            .layer
                            .unwrap()
                    },
                },
            );
        }
        let MediaEditResult::MatrixCreated { items, .. } = matrix(
            &mut engine,
            MatrixEdit::AddGroup {
                parent: None,
                label: Some("Outer Layer scope".into()),
            },
        ) else {
            panic!()
        };
        let MatrixItemId::Group { id: layer_group } = items[0] else {
            panic!()
        };
        let MediaEditResult::MatrixCreated { items, .. } = matrix(
            &mut engine,
            MatrixEdit::AddGroup {
                parent: Some(layer_group),
                label: Some("Inner Layer scope".into()),
            },
        ) else {
            panic!()
        };
        let MatrixItemId::Group { id: nested } = items[0] else {
            panic!()
        };
        matrix(
            &mut engine,
            MatrixEdit::MoveItem {
                item: MatrixItemId::Layer { id: rows[0] },
                parent: Some(nested),
                after: None,
            },
        );
        let MediaEditResult::MatrixColumnGroupCreated { group: column_group } = matrix(
            &mut engine,
            MatrixEdit::AddColumnGroup {
                parent: None,
                label: Some("Outer Column scope".into()),
            },
        ) else {
            panic!()
        };
        let MediaEditResult::MatrixColumnGroupCreated { group: nested } = matrix(
            &mut engine,
            MatrixEdit::AddColumnGroup {
                parent: Some(column_group),
                label: Some("Inner Column scope".into()),
            },
        ) else {
            panic!()
        };
        matrix(
            &mut engine,
            MatrixEdit::MoveColumnItem {
                item: ColumnItemId::Column { id: columns[1] },
                parent: Some(nested),
                after: None,
            },
        );
        let mut cells = Vec::new();
        for (i, rgba) in [[1.0, 0.0, 0.0, 1.0], [0.0, 0.0, 1.0, 1.0]].into_iter().enumerate() {
            let MediaEditResult::Created { reference } = edit(
                &mut engine,
                MediaEdit::CreateMedia {
                    media_kind: MediaKind::SolidColor,
                    label: None,
                },
            ) else {
                panic!()
            };
            edit(
                &mut engine,
                MediaEdit::ConfigureChannel {
                    reference,
                    source: ChannelSource::SolidColor { rgba },
                },
            );
            edit(
                &mut engine,
                MediaEdit::ConfigureMediaResolution {
                    media: reference.media,
                    size: SizePolicy::Fixed {
                        extent: Extent2D::new(16, 24).unwrap(),
                    },
                },
            );
            let MediaEditResult::MatrixMediaCreated { cell, .. } = matrix(
                &mut engine,
                MatrixEdit::DropSource {
                    layer: rows[i],
                    column: columns[i + 1],
                    source: reference.media.0,
                    copy: false,
                },
            ) else {
                panic!()
            };
            cells.push(cell);
            matrix(
                &mut engine,
                MatrixEdit::ConfigureCell {
                    layer: rows[i],
                    column: columns[i + 1],
                    settings: CellSettings {
                        position: [i as f32 * 16.0, 0.0],
                        ..Default::default()
                    },
                },
            );
            if let Some(definition) = application.clone() {
                matrix(
                    &mut engine,
                    MatrixEdit::ConfigureCellApplication {
                        layer: rows[i],
                        column: columns[i + 1],
                        definition,
                        pre: 0,
                        post: 0,
                    },
                );
            }
        }
        Self {
            engine,
            root,
            rows: [rows[0], rows[1]],
            columns,
            cells: cells.try_into().unwrap(),
            layer_group,
            column_group,
        }
    }
    pub fn matrix(&mut self, operation: MatrixEdit) -> MediaEditResult {
        edit(
            &mut self.engine,
            MediaEdit::Matrix {
                media: self.root.media,
                edit: operation,
            },
        )
    }
    pub fn play(&mut self) {
        self.matrix(MatrixEdit::ClearPlayback);
        self.matrix(MatrixEdit::LaunchColumn {
            column: self.columns[0],
            mode: LaunchMode::Cut,
        });
        for i in 0..2 {
            self.matrix(MatrixEdit::LaunchCell {
                layer: self.rows[i],
                column: self.columns[i + 1],
                mode: LaunchMode::Cut,
            });
        }
    }
    pub fn owner(&self, scope: Scope) -> NodeUuid {
        match scope {
            Scope::Cell => self.cells[0].0,
            Scope::Layer => self.rows[0].0,
            Scope::LayerGroup => self.layer_group.0,
            Scope::Column => self.columns[1].0,
            Scope::ColumnGroup => self.column_group.0,
            Scope::All => self.root.media.0,
        }
    }
    pub fn parameter(&self, scope: Scope) -> NodeId {
        let owner = self.engine.node_id_by_uuid(self.owner(scope)).unwrap();
        let folder = self
            .engine
            .nodes
            .iter()
            .find(|(_, node)| node.node_data().parent == Some(owner) && node.node_data().meta.decl_id.0 == "parameters")
            .unwrap()
            .0;
        let field = if scope == Scope::All {
            "parameters/stop_all"
        } else {
            "parameters/stop"
        };
        self.engine
            .nodes
            .iter()
            .find(|(_, node)| node.node_data().parent == Some(folder) && node.node_data().meta.decl_id.0 == field)
            .unwrap()
            .0
    }
    pub fn stop(&mut self, scope: Scope, caller: Caller) {
        let saved = to_sparse_project_json_pretty(&self.engine).unwrap();
        let history = self.engine.undo_len();
        let node = self.parameter(scope);
        match caller {
            Caller::Transport => {
                let ack = apply_project_intent(
                    &mut self.engine,
                    UiEditIntent::SetParam {
                        node,
                        value: ParamValue::Trigger(),
                        behaviour: ParameterEventBehaviour::Append,
                    },
                );
                assert!(ack.success, "{scope:?}: {ack:?}");
            }
            Caller::Script => {
                self.engine.edits.push(Edit::SetNodeScriptProperty {
                    node,
                    write: ScriptPropertyWrite::prepare("value", ParamValue::Trigger()).unwrap(),
                });
                for _ in 0..2 {
                    self.engine.apply_edits().unwrap();
                    self.engine
                        .dispatch_inbox(ExecutionPhase::EndOfTickStabilization)
                        .unwrap();
                }
                self.engine.apply_edits().unwrap();
            }
        }
        service_script_app_operations(&mut self.engine).unwrap();
        assert_eq!(self.engine.undo_len(), history);
        assert_eq!(to_sparse_project_json_pretty(&self.engine).unwrap(), saved);
    }
    pub fn playback(&mut self) -> MatrixPlaybackSnapshot {
        let MediaEditResult::MatrixPlayback { snapshot } = self.matrix(MatrixEdit::InspectPlayback) else {
            panic!()
        };
        snapshot
    }
    pub fn check_stopped(&mut self, scope: Scope) {
        let state = self.playback();
        assert_eq!(state.desired_column, (scope != Scope::All).then_some(self.columns[0]));
        assert!(
            state
                .layers
                .iter()
                .all(|entry| entry.layer != self.rows[0] || entry.column.is_none())
        );
        if scope == Scope::All {
            assert!(state.layers.is_empty());
        } else {
            assert_eq!(
                state
                    .layers
                    .iter()
                    .find(|entry| entry.layer == self.rows[1])
                    .unwrap()
                    .column,
                Some(self.columns[2])
            );
        }
    }
}

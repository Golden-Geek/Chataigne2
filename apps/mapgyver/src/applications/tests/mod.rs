use super::{demand::*, protocol::*};
use crate::{
    app::{AppEngine, AppNode},
    matrix::protocol::*,
    media::protocol::*,
};
use golden_core::{
    app::{create_new_project_engine, from_sparse_project_json, to_sparse_project_json_pretty},
    application::apply_project_intent,
    node::Node,
    ui_sync::UiEditIntent,
};

mod availability;
mod demand;
mod headless;
mod inspection;
mod logical;
mod parameters;

pub(crate) fn fixture() -> AppEngine {
    let mut engine = create_new_project_engine().unwrap();
    engine.apply_edits().unwrap();
    engine
}
pub(crate) fn invoke(engine: &mut AppEngine, edit: MediaEdit) -> golden_core::ui_sync::UiAck {
    apply_project_intent(
        engine,
        UiEditIntent::InvokeAppOperation {
            namespace: MEDIA_NAMESPACE.into(),
            operation: MEDIA_OPERATION.into(),
            payload: serde_json::to_value(edit).unwrap(),
        },
    )
}
pub(crate) fn edit(engine: &mut AppEngine, edit: MediaEdit) -> MediaEditResult {
    let ack = invoke(engine, edit);
    assert!(ack.success, "{ack:?}");
    serde_json::from_value(ack.result.unwrap().value).unwrap()
}
pub(crate) fn app_edit(engine: &mut AppEngine, edit: ApplicationEdit) -> MediaEditResult {
    self::edit(engine, MediaEdit::Applications { edit })
}
pub(crate) fn matrix(engine: &mut AppEngine) -> (MediaId, Vec<ColumnId>) {
    let MediaEditResult::Created { reference } = edit(
        engine,
        MediaEdit::CreateMedia {
            media_kind: MediaKind::Matrix,
            label: None,
        },
    ) else {
        panic!()
    };
    let MediaEditResult::MatrixCreated { columns, .. } = edit(
        engine,
        MediaEdit::Matrix {
            media: reference.media,
            edit: MatrixEdit::AddColumns { parent: None, count: 5 },
        },
    ) else {
        panic!()
    };
    (reference.media, columns)
}
pub(crate) fn create(engine: &mut AppEngine) -> ApplicationId {
    let (_, columns) = matrix(engine);
    ApplicationId(columns[2].0)
}
pub(crate) fn owner(engine: &AppEngine, application: ApplicationId) -> (MediaId, ColumnId) {
    let node = engine
        .nodes
        .get(engine.node_id_by_uuid(application.0).unwrap())
        .unwrap();
    let root = engine.nodes.get(node.node_data().parent.unwrap()).unwrap();
    let media = engine.nodes.get(root.node_data().parent.unwrap()).unwrap();
    (MediaId(media.node_data().meta.uuid), ColumnId(application.0))
}
pub(crate) fn remove(engine: &mut AppEngine, application: ApplicationId) {
    let (media, column) = owner(engine, application);
    edit(
        engine,
        MediaEdit::Matrix {
            media,
            edit: MatrixEdit::RemoveColumn { column },
        },
    );
}
pub(crate) fn enable(engine: &mut AppEngine, application: ApplicationId, enabled: bool) {
    let node = engine.node_id_by_uuid(application.0).unwrap();
    let ack = apply_project_intent(
        engine,
        UiEditIntent::PatchMeta {
            node,
            patch: golden_core::ui_sync::NodeMetaPatch {
                enabled: Some(enabled),
                ..Default::default()
            },
        },
    );
    assert!(ack.success, "{ack:?}");
}
pub(crate) fn configure_column(
    engine: &mut AppEngine,
    media: MediaId,
    column: ColumnId,
    application: ApplicationId,
) -> ApplicationId {
    let mut definition = super::inspect(engine, application).unwrap().definition;
    if definition.executable.is_none() {
        definition.executable = Some("C:\\mapgyver-fixture-missing\\never.exe".into());
    }
    edit(
        engine,
        MediaEdit::Matrix {
            media,
            edit: MatrixEdit::ConfigureColumnApplication {
                column,
                definition,
                pre: 1,
                post: 1,
            },
        },
    );
    ApplicationId(column.0)
}

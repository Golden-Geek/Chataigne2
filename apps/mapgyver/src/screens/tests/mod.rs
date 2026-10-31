mod authoring;
mod commands;
mod geometry;
mod gestures;
mod typescript;

use super::protocol::*;
use crate::app::AppEngine;
use golden_core::{
    app::create_new_project_engine,
    application::apply_project_intent,
    ui_sync::{UiAck, UiEditIntent},
};

fn fixture() -> AppEngine {
    let mut engine = create_new_project_engine().unwrap();
    engine.apply_edits().unwrap();
    engine
}

fn invoke(engine: &mut AppEngine, edit: ScreenEdit) -> UiAck {
    apply_project_intent(
        engine,
        UiEditIntent::InvokeAppOperation {
            namespace: SCREEN_NAMESPACE.into(),
            operation: SCREEN_OPERATION.into(),
            payload: serde_json::to_value(edit).unwrap(),
        },
    )
}

fn apply(engine: &mut AppEngine, edit: ScreenEdit) -> ScreenEditResult {
    let ack = invoke(engine, edit);
    assert!(ack.success, "{ack:?}");
    serde_json::from_value(ack.result.unwrap().value).unwrap()
}

fn create(engine: &mut AppEngine) -> (ScreenId, SurfaceReference) {
    let ScreenEditResult::ScreenCreated { screen, surface } = apply(engine, ScreenEdit::CreateScreen { label: None })
    else {
        panic!()
    };
    (screen, surface)
}

fn definition(engine: &AppEngine, reference: SurfaceReference) -> SurfaceDefinition {
    super::inspect(engine)
        .unwrap()
        .screens
        .into_iter()
        .find(|screen| screen.id == reference.screen)
        .unwrap()
        .surfaces
        .into_iter()
        .find(|surface| surface.reference == reference)
        .unwrap()
        .definition
}

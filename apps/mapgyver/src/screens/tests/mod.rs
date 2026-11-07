mod anti_aliasing;
mod authoring;
mod catalog_runtime;
mod creation;
mod crop_gestures;
mod drop_sources;
mod editor;
mod enabled;
mod fit;
mod free_warp;
mod geometry;
mod geometry_parameters;
mod gestures;
mod output_input;
mod parameters;
mod pixel_gestures;
mod queries;
mod rows;
mod scripting;
mod selection;
mod translation;
mod typescript;

use super::protocol::*;
use crate::app::AppEngine;
use golden_core::{
    app::create_new_project_engine,
    application::apply_project_intent,
    ui_sync::{UiAck, UiEditIntent},
};

pub(super) fn fixture() -> AppEngine {
    let mut engine = create_new_project_engine().unwrap();
    engine.apply_edits().unwrap();
    // These domain-operation suites deliberately start with an empty manager.
    // Fresh-project seeding is covered separately by the app lifecycle tests.
    let initial = super::inspect(&engine).unwrap().screens.pop().unwrap();
    apply(&mut engine, ScreenEdit::RemoveScreen { screen: initial.id });
    engine.clear_history();
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

pub(super) fn apply(engine: &mut AppEngine, edit: ScreenEdit) -> ScreenEditResult {
    let ack = invoke(engine, edit);
    assert!(ack.success, "{ack:?}");
    serde_json::from_value(ack.result.unwrap().value).unwrap()
}

pub(super) fn create(engine: &mut AppEngine) -> (ScreenId, SurfaceReference) {
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

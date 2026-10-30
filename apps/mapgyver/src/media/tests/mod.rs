mod assets;
mod authoring;
mod catalog;
mod picture_import;
mod resolution;
mod script;
mod typescript;

use golden_core::app::create_new_project_engine;
use golden_core::application::apply_project_intent;
use golden_core::ui_sync::{UiAck, UiEditIntent};

use super::protocol::*;
use crate::app::AppEngine;

fn fixture() -> AppEngine {
    let mut engine = create_new_project_engine().unwrap();
    engine.apply_edits().unwrap();
    engine
}

fn invoke(engine: &mut AppEngine, edit: MediaEdit) -> UiAck {
    apply_project_intent(
        engine,
        UiEditIntent::InvokeAppOperation {
            namespace: MEDIA_NAMESPACE.into(),
            operation: MEDIA_OPERATION.into(),
            payload: serde_json::to_value(edit).unwrap(),
        },
    )
}

fn apply(engine: &mut AppEngine, edit: MediaEdit) -> MediaEditResult {
    let ack = invoke(engine, edit);
    assert!(ack.success, "{ack:?}");
    serde_json::from_value(ack.result.unwrap().value).unwrap()
}

fn create(engine: &mut AppEngine, kind: MediaKind) -> ChannelReference {
    let MediaEditResult::Created { reference } = apply(
        engine,
        MediaEdit::CreateMedia {
            media_kind: kind,
            label: None,
        },
    ) else {
        panic!("created result")
    };
    reference
}

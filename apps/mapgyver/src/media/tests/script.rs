use super::*;
use golden_core::application::service_script_app_operations;
use golden_core::events::EventKind;
use golden_core::node::Node;
use golden_core::script::{ScriptNode, ScriptNodeConfig, ScriptSource};

#[test]
fn quickjs_uses_the_same_backend_defaults_committed_identity_and_undo() {
    let mut engine = fixture();
    let request = serde_json::to_string(&MediaEdit::CreateMedia {
        media_kind: MediaKind::SolidColor,
        label: None,
    })
    .unwrap();
    let source = format!(
        "function init() {{ gc.invokeAppOperation('create', {MEDIA_NAMESPACE:?}, {MEDIA_OPERATION:?}, {request}); }}\nfunction onAppOperationResult(reply) {{ gc.emit('media-reply', reply); }}"
    );
    let mut script = ScriptNode::new(
        "Caller",
        ScriptNodeConfig {
            source: ScriptSource::Inline { text: source },
        },
    );
    script.budgets.max_wall_time_us_per_load = 1_000_000;
    script.budgets.max_wall_time_us_per_callback = 1_000_000;
    engine.add_node(script.into(), None);
    engine.apply_edits().unwrap();
    let before = engine.undo_len();
    service_script_app_operations(&mut engine).unwrap();
    let reply = engine
        .ui_event_log()
        .iter()
        .find_map(|event| match &event.kind {
            EventKind::Custom(event) if event.topic == "media-reply" => Some((*event.payload).clone()),
            _ => None,
        })
        .expect("committed callback");
    assert_eq!(reply["status"], "applied");
    let result: MediaEditResult = serde_json::from_value(reply["value"].clone()).unwrap();
    let MediaEditResult::Created { reference } = result else {
        panic!()
    };
    assert_eq!(
        crate::media::channel(&engine, reference).unwrap().definition,
        ChannelDefinition::for_media(MediaKind::SolidColor)
    );
    assert_eq!(engine.undo_len(), before + 1);
    assert!(engine.undo().unwrap());
    assert!(crate::media::channel(&engine, reference).is_err());
    assert!(engine.redo().unwrap());
    assert_eq!(
        engine
            .nodes
            .get(engine.node_id_by_uuid(reference.channel.0).unwrap())
            .unwrap()
            .node_data()
            .meta
            .uuid,
        reference.channel.0
    );
}

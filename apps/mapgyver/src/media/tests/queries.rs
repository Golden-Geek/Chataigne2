use super::*;
use crate::app::AppNode;
use golden_core::app::{ProjectLifecycle, to_sparse_project_json_pretty};

#[test]
fn exact_channel_reads_are_nullable_after_deletion_and_restore_with_undo_without_read_history() {
    let mut engine = fixture();
    let source = create(&mut engine, MediaKind::SolidColor);
    let MediaEditResult::Created { reference: peer } = apply(
        &mut engine,
        MediaEdit::AddChannel {
            media: source.media,
            label: None,
        },
    ) else {
        panic!()
    };
    apply(&mut engine, MediaEdit::RemoveChannel { reference: peer });
    for reference in [
        peer,
        ChannelReference {
            media: source.media,
            channel: ChannelId(source.media.0),
        },
    ] {
        let history = engine.ui_history_state();
        let before = to_sparse_project_json_pretty(&engine).unwrap();
        let payload = serde_json::to_value(MediaEdit::InspectChannel { reference }).unwrap();
        let query = AppNode::query_app_operation(&engine, MEDIA_NAMESPACE, MEDIA_OPERATION, &payload).unwrap();
        let reply = invoke(&mut engine, MediaEdit::InspectChannel { reference });
        if reference == peer {
            let query = query.unwrap();
            assert!(reply.success);
            assert_eq!(query, reply.result.unwrap().value);
            assert!(query["snapshot"].is_null());
        } else {
            assert!(query.is_err(), "wrong existing node types must still reject");
            assert!(!reply.success);
        }
        assert_eq!(engine.ui_history_state(), history);
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
    }
    assert!(engine.undo().unwrap());
    let restored = invoke(&mut engine, MediaEdit::InspectChannel { reference: peer });
    assert_eq!(
        restored.result.unwrap().value["snapshot"]["reference"],
        serde_json::to_value(peer).unwrap()
    );
    apply(&mut engine, MediaEdit::RemoveMedia { media: source.media });
    let history = engine.ui_history_state();
    let missing = invoke(&mut engine, MediaEdit::InspectChannel { reference: source });
    assert!(missing.success);
    assert!(missing.result.unwrap().value["snapshot"].is_null());
    assert_eq!(engine.ui_history_state(), history);
}

#[test]
fn immutable_channel_query_and_project_intent_return_the_same_authored_page_and_rejections() {
    let mut engine = fixture();
    let source = create(&mut engine, MediaKind::SolidColor);
    let MediaEditResult::ReferenceCreated { media } = apply(
        &mut engine,
        MediaEdit::CreateReference {
            target: Some(source.media),
            label: None,
        },
    ) else {
        panic!()
    };
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.ui_history_state();
    for count in [1, 32, 129] {
        let edit = MediaEdit::InspectChannels {
            query: MediaChannelsQuery { media, start: 0, count },
        };
        let payload = serde_json::to_value(&edit).unwrap();
        let query = AppNode::query_app_operation(&engine, MEDIA_NAMESPACE, MEDIA_OPERATION, &payload).unwrap();
        let reply = invoke(&mut engine, edit);
        match query {
            Ok(value) => {
                assert!(reply.success);
                assert_eq!(reply.result.unwrap().value, value);
                let MediaEditResult::Channels { page } = serde_json::from_value(value).unwrap() else {
                    panic!()
                };
                assert_eq!(page.total, 1);
                assert_eq!(
                    page.channels[0].reference,
                    crate::media::resolve_source_node(&engine, media.0).unwrap()
                );
            }
            Err(_) => {
                assert!(!reply.success);
                assert!(reply.result.is_none());
            }
        }
        assert_eq!(engine.ui_history_state(), history);
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
    }
    let mutation = serde_json::to_value(MediaEdit::CreateReference {
        target: None,
        label: None,
    })
    .unwrap();
    assert!(AppNode::query_app_operation(&engine, MEDIA_NAMESPACE, MEDIA_OPERATION, &mutation).is_none());
    assert!(AppNode::query_app_operation(&engine, "other", MEDIA_OPERATION, &mutation).is_none());
    assert!(AppNode::query_app_operation(&engine, MEDIA_NAMESPACE, "other", &mutation).is_none());
}

use super::*;
use crate::app::AppNode;
use golden_core::app::{ProjectLifecycle, to_sparse_project_json_pretty};

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
                    ChannelReference {
                        media,
                        channel: source.channel
                    }
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

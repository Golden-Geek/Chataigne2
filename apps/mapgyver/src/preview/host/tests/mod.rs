use super::Request;
use crate::media::protocol::MediaNativeRequest;
use golden_video::preview::protocol::{PREVIEW_PROTOCOL_VERSION, PreviewRequest};

#[test]
fn discovery_and_all_existing_preview_requests_keep_their_canonical_contracts() {
    let id = uuid::Uuid::new_v4();
    let requests = [
        PreviewRequest::Connect {
            client: id,
            version: PREVIEW_PROTOCOL_VERSION,
        },
        PreviewRequest::Layout {
            session: id,
            revision: 1,
            views: vec![],
        },
        PreviewRequest::Status {
            session: id,
            revision: 1,
        },
        PreviewRequest::Disconnect { session: id },
    ];
    for request in requests {
        let json = serde_json::to_string(&request).unwrap();
        let Request::Preview(decoded) = serde_json::from_str(&json).unwrap() else {
            panic!("preview route")
        };
        assert_eq!(
            serde_json::to_value(decoded).unwrap(),
            serde_json::to_value(request).unwrap()
        );
    }
    let json = serde_json::to_string(&MediaNativeRequest::Senders { refresh: true }).unwrap();
    assert!(matches!(
        serde_json::from_str::<Request>(&json).unwrap(),
        Request::Media(MediaNativeRequest::Senders { refresh: true })
    ));
    assert!(serde_json::from_str::<Request>(r#"{"kind":"mapgyverSenders","refresh":"invalid"}"#).is_err());
    let request = crate::screens::protocol::ScreenNativeRequest::Displays { refresh: true };
    let json = serde_json::to_string(&request).unwrap();
    assert!(matches!(
        serde_json::from_str::<Request>(&json).unwrap(),
        Request::Screen(_)
    ));
}

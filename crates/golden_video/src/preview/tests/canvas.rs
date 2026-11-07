use super::*;

#[test]
fn content_black_and_overlay_clip_are_independent_and_theme_changes_replace_the_plan() {
    let (control, reader, session) = fixture();
    let mut image = view();
    image.rect = CssRect {
        x: 5.,
        y: 5.,
        width: 10.,
        height: 10.,
    };
    image.clip = CssRect {
        x: 0.,
        y: 0.,
        width: 30.,
        height: 20.,
    };
    layout(&control, session, 1, vec![image.clone()]);
    let first = reader.snapshot();
    let content = first.views[0].destination;
    assert_ne!(content, first.views[0].backdrop);
    assert_eq!(
        first.backdrops(crate::TextureId::new(1).unwrap())[0].destination,
        content
    );
    assert_eq!(first.background, crate::LinearColor::from_srgba8([21, 21, 21, 255]));
    let reply = control.handle(
        PreviewRequest::Layout {
            session,
            revision: 2,
            views: vec![image],
            background: [88, 90, 94, 255],
        },
        host(),
    );
    assert!(matches!(reply, PreviewResponse::Applied { .. }));
    let themed = reader.snapshot();
    assert_eq!(themed.background, crate::LinearColor::from_srgba8([88, 90, 94, 255]));
    assert_ne!(themed.routing_generation, first.routing_generation);
    assert!(!first.routing_guard.is_current());
    assert_eq!(themed.views[0].destination, content);
    assert_eq!(themed.views[0].backdrop, first.views[0].backdrop);
}

#[test]
fn canvas_colour_is_bounded_wire_metadata_and_old_clients_reject() {
    let (control, _, _) = fixture();
    assert!(matches!(
        control.handle(
            PreviewRequest::Connect {
                client: Uuid::new_v4(),
                version: 6
            },
            host()
        ),
        PreviewResponse::Rejected {
            code: PreviewRejection::Version
        }
    ));
    let invalid = serde_json::json!({ "kind": "Layout", "session": Uuid::new_v4(), "revision": 1, "views": [], "background": [256, 0, 0, 255] });
    assert!(serde_json::from_value::<PreviewRequest>(invalid).is_err());
}

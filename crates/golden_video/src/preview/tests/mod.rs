use super::*;
use protocol::{CssRect, PreviewView};

fn host() -> HostViewport {
    HostViewport {
        client_width: 100,
        client_height: 100,
        webview_origin_x: 10,
        webview_origin_y: 20,
        webview_width: 80,
        webview_height: 60,
        css_scale: 2.,
    }
}
fn view() -> PreviewView {
    PreviewView {
        id: Uuid::new_v4(),
        source: "channel/a".into(),
        rect: CssRect {
            x: -5.,
            y: -10.,
            width: 30.,
            height: 20.,
        },
        clip: CssRect {
            x: 0.,
            y: 0.,
            width: 20.,
            height: 10.,
        },
        visible: true,
    }
}
fn fixture() -> (PreviewController, PreviewReceiver, Uuid) {
    let (control, reader) = PreviewController::new(PreviewLimits::default(), None).unwrap();
    let PreviewResponse::Connected { session, .. } = control.handle(
        PreviewRequest::Connect {
            client: Uuid::new_v4(),
            version: 1,
        },
        host(),
    ) else {
        panic!()
    };
    (control, reader, session)
}
fn layout(control: &PreviewController, session: Uuid, revision: u32, views: Vec<PreviewView>) -> PreviewResponse {
    control.handle(
        PreviewRequest::Layout {
            session,
            revision,
            views,
        },
        host(),
    )
}
fn close(left: f32, right: f32) {
    assert!((left - right).abs() < 0.00001, "{left} != {right}");
}

#[test]
fn host_scale_origin_and_clip_preserve_source_crop_in_normalized_backing_space() {
    let (control, reader, session) = fixture();
    assert_eq!(
        layout(&control, session, 1, vec![view()]),
        PreviewResponse::Applied {
            revision: 1,
            visible_views: 1,
            installed: false
        }
    );
    let snapshot = reader.snapshot();
    let image = &snapshot.views[0];
    assert!(reader.mark_installed(snapshot.generation));
    assert!(reader.mark_uninstalled(snapshot.generation));
    assert!(matches!(
        control.handle(PreviewRequest::Status { session, revision: 1 }, host()),
        PreviewResponse::Applied { installed: false, .. }
    ));
    assert!(reader.mark_installed(snapshot.generation));
    assert_eq!(
        control.handle(PreviewRequest::Status { session, revision: 1 }, host()),
        PreviewResponse::Applied {
            revision: 1,
            visible_views: 1,
            installed: true
        }
    );
    close(image.destination.x, 0.1);
    close(image.destination.y, 0.2);
    close(image.destination.width, 0.4);
    close(image.destination.height, 0.2);
    close(image.source_rect.x, 1. / 6.);
    close(image.source_rect.y, 0.5);
    close(image.source_rect.width, 2. / 3.);
    close(image.source_rect.height, 0.5);
    let regions = snapshot.regions(|key| (key == "channel/a").then(|| TextureId::new(7).unwrap()));
    assert_eq!(regions.len(), 1);
    assert_eq!(regions[0].source.get(), 7);
    assert!(
        snapshot.regions(|_| None).is_empty(),
        "missing sources cannot reuse another view's texture"
    );
    let mut resized = host();
    resized.css_scale = 1.5;
    assert_eq!(
        control.handle(PreviewRequest::Status { session, revision: 1 }, resized),
        PreviewResponse::Rejected {
            code: PreviewRejection::Geometry
        }
    );
    assert!(
        !snapshot.guard.is_current(),
        "new native scale must withhold readiness until a fresh layout"
    );
}

#[test]
fn latest_full_layout_coalesces_and_heartbeats_do_not_invalidate_prepared_geometry() {
    let (control, reader, session) = fixture();
    let mut source = view();
    layout(&control, session, 1, vec![source.clone()]);
    let initial = reader.snapshot();
    for revision in 2..=100 {
        assert!(matches!(
            layout(&control, session, revision, vec![source.clone()]),
            PreviewResponse::Applied { .. }
        ));
    }
    assert_eq!(reader.snapshot().generation, initial.generation);
    assert!(initial.guard.is_current());
    source.visible = false;
    layout(&control, session, 101, vec![source]);
    assert!(!initial.guard.is_current());
    assert!(reader.snapshot().views.is_empty());
    assert!(
        !reader.mark_installed(initial.generation),
        "old installations must not expose a newer DOM hole"
    );
    assert!(matches!(
        layout(&control, session, 100, vec![view()]),
        PreviewResponse::Rejected {
            code: PreviewRejection::Revision
        }
    ));
    assert!(reader.snapshot().views.is_empty());
}

#[test]
fn replacing_session_and_unregister_fence_late_preparation_without_clearing_new_views() {
    let (control, reader, old) = fixture();
    layout(&control, old, 1, vec![view()]);
    let initial = reader.snapshot();
    let PreviewResponse::Connected { session, .. } = control.handle(
        PreviewRequest::Connect {
            client: Uuid::new_v4(),
            version: 1,
        },
        host(),
    ) else {
        panic!()
    };
    assert!(!initial.guard.is_current());
    layout(&control, session, 1, vec![view()]);
    let current = reader.snapshot();
    assert_eq!(
        layout(&control, old, 2, Vec::new()),
        PreviewResponse::Rejected {
            code: PreviewRejection::Session
        }
    );
    assert!(current.guard.is_current());
    assert_eq!(reader.snapshot().views.len(), 1);
    layout(&control, session, 2, Vec::new());
    assert!(!current.guard.is_current());
    assert!(reader.snapshot().views.is_empty());
    control.handle(PreviewRequest::Disconnect { session }, host());
    assert!(reader.next_deadline().is_none());
}

#[test]
fn invalid_geometry_capacity_sources_and_duplicate_ids_fail_closed_for_current_session() {
    let (control, reader, session) = fixture();
    let cases = [
        (PreviewRejection::Geometry, {
            let mut v = view();
            v.rect.x = f64::NAN;
            vec![v]
        }),
        (PreviewRejection::Source, {
            let mut v = view();
            v.source = "x".repeat(256);
            vec![v]
        }),
        (PreviewRejection::Duplicate, {
            let v = view();
            vec![v.clone(), v]
        }),
        (PreviewRejection::Capacity, (0..65).map(|_| view()).collect()),
    ];
    for (index, (error, views)) in cases.into_iter().enumerate() {
        layout(&control, session, index as u32 * 2, vec![view()]);
        let old = reader.snapshot();
        assert_eq!(
            layout(&control, session, index as u32 * 2 + 1, views),
            PreviewResponse::Rejected { code: error }
        );
        assert!(!old.guard.is_current());
        assert!(reader.snapshot().views.is_empty());
    }
}

#[test]
fn lease_expiry_clears_disconnected_views_and_close_prevents_new_registration() {
    let (control, reader) = PreviewController::new(
        PreviewLimits {
            max_views: 2,
            lease: Duration::from_millis(100),
        },
        None,
    )
    .unwrap();
    let PreviewResponse::Connected { session, .. } = control.handle(
        PreviewRequest::Connect {
            client: Uuid::new_v4(),
            version: 1,
        },
        host(),
    ) else {
        panic!()
    };
    layout(&control, session, 1, vec![view()]);
    let old = reader.snapshot();
    std::thread::sleep(Duration::from_millis(125));
    assert!(reader.snapshot().views.is_empty());
    assert!(!old.guard.is_current());
    assert_eq!(
        layout(&control, session, 2, vec![view()]),
        PreviewResponse::Rejected {
            code: PreviewRejection::Session
        }
    );
    control.close();
    assert_eq!(
        control.handle(
            PreviewRequest::Connect {
                client: Uuid::new_v4(),
                version: 1
            },
            host()
        ),
        PreviewResponse::Rejected {
            code: PreviewRejection::Closed
        }
    );
}

#[test]
fn wire_contract_rejects_mismatched_version_and_contains_only_layout_metadata() {
    let (control, _, session) = fixture();
    assert_eq!(
        control.handle(
            PreviewRequest::Connect {
                client: Uuid::new_v4(),
                version: 2
            },
            host()
        ),
        PreviewResponse::Rejected {
            code: PreviewRejection::Version
        }
    );
    let request = serde_json::to_string(&PreviewRequest::Layout {
        session,
        revision: 1,
        views: vec![view()],
    })
    .unwrap();
    let response: PreviewResponse = serde_json::from_str(&control.control_json(&request, host()).unwrap()).unwrap();
    assert!(matches!(response, PreviewResponse::Applied { visible_views: 1, .. }));
    for forbidden in ["pixels", "texture", "handle", "devicePixelRatio"] {
        assert!(!request.contains(forbidden));
    }
    assert!(control.control_json(&"x".repeat(65537), host()).is_err());
}

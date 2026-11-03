use super::*;

fn at(x: f64) -> PreviewView {
    let mut result = view();
    result.rect = CssRect {
        x,
        y: 0.,
        width: 10.,
        height: 10.,
    };
    result.clip = CssRect {
        x: 0.,
        y: 0.,
        width: 40.,
        height: 30.,
    };
    result
}

fn guard(snapshot: &PreviewSnapshot, index: usize) -> RevisionGuard {
    snapshot.regions(crate::OutputId::new(1).unwrap(), |_| Some(TextureId::new(1).unwrap()))[index]
        .visibility
        .clone()
        .unwrap()
}

#[test]
fn source_swaps_and_unrelated_registration_keep_live_destinations_until_replacement() {
    let (control, reader, session) = fixture();
    let mut a = at(0.);
    let b = at(20.);
    layout(&control, session, 1, vec![a.clone()]);
    let initial = reader.snapshot();
    let a_guard = guard(&initial, 0);
    layout(&control, session, 2, vec![a.clone(), b.clone()]);
    let expanded = reader.snapshot();
    let b_guard = guard(&expanded, 1);
    assert!(
        a_guard.is_current(),
        "adding an Inspector preview must not clear the editor"
    );
    assert!(!initial.guard.is_current(), "stale full layouts still cannot install");
    assert!(!reader.mark_installed(initial.generation));
    a.source = "channel/replacement".into();
    layout(&control, session, 3, vec![a.clone(), b]);
    assert!(
        a_guard.is_current(),
        "the prior source may render until its replacement is installed"
    );
    assert!(b_guard.is_current());
    layout(&control, session, 4, vec![a.clone()]);
    assert!(a_guard.is_current());
    assert!(
        !b_guard.is_current(),
        "unregistered destinations cannot leave stale pixels"
    );
    a.rect.x = 2.;
    layout(&control, session, 5, vec![a]);
    assert!(
        a_guard.is_current(),
        "pan retains the installed frame until its replacement is ready"
    );
}

#[test]
fn zoom_retains_frames_but_host_session_and_lease_changes_withdraw_captured_destinations() {
    let (control, reader, session) = fixture();
    let mut a = at(0.);
    a.rect.width = 80.;
    layout(&control, session, 1, vec![a.clone()]);
    let initial = reader.snapshot();
    let before_crop = guard(&initial, 0);
    // Destination remains clip-limited while the sampled source crop changes.
    a.rect.width = 100.;
    layout(&control, session, 2, vec![a.clone()]);
    assert_eq!(initial.views[0].destination, reader.snapshot().views[0].destination);
    assert!(before_crop.is_current());
    assert!(!reader.mark_installed(initial.generation));
    let before_host = guard(&reader.snapshot(), 0);
    let mut resized = host();
    resized.client_width *= 2;
    control.handle(
        PreviewRequest::Layout {
            session,
            revision: 3,
            views: vec![a.clone()],
        },
        resized,
    );
    assert!(!before_host.is_current());
    let before_session = guard(&reader.snapshot(), 0);
    let PreviewResponse::Connected { session, .. } = control.handle(
        PreviewRequest::Connect {
            client: Uuid::new_v4(),
            version: PREVIEW_PROTOCOL_VERSION,
        },
        host(),
    ) else {
        panic!()
    };
    assert!(!before_session.is_current());
    layout(&control, session, 1, vec![a]);
    let before_expiry = guard(&reader.snapshot(), 0);
    control.0.state.lock().unwrap().session.as_mut().unwrap().deadline = Instant::now();
    assert!(reader.snapshot().views.is_empty());
    assert!(!before_expiry.is_current());
    let PreviewResponse::Connected { session, .. } = control.handle(
        PreviewRequest::Connect {
            client: Uuid::new_v4(),
            version: PREVIEW_PROTOCOL_VERSION,
        },
        host(),
    ) else {
        panic!()
    };
    layout(&control, session, 1, vec![at(0.)]);
    let before_close = guard(&reader.snapshot(), 0);
    control.close();
    assert!(!before_close.is_current());
}

#[test]
fn overlapping_painter_order_changes_withdraw_only_affected_destinations() {
    for (x, should_remain) in [(5., false), (20., true)] {
        let (control, reader, session) = fixture();
        let a = at(0.);
        let b = at(x);
        layout(&control, session, 1, vec![a.clone(), b.clone()]);
        let old = reader.snapshot();
        layout(&control, session, 2, vec![b, a]);
        for index in 0..2 {
            assert_eq!(guard(&old, index).is_current(), should_remain);
        }
    }
}

#[test]
fn sustained_pan_zoom_moves_an_installed_region_without_replacing_resource_routing() {
    let (control, reader, session) = fixture();
    let mut a = at(0.);
    layout(&control, session, 1, vec![a.clone()]);
    let installed = reader.snapshot();
    let region = installed
        .regions(crate::OutputId::new(1).unwrap(), |_| Some(TextureId::new(1).unwrap()))
        .remove(0);
    let backing = installed.backdrops(TextureId::new(1).unwrap()).remove(0);
    let device = crate::runtime::DeviceGeneration::new(Uuid::new_v4(), 1);
    for step in 2..100 {
        a.rect.x = f64::from(step % 10);
        a.rect.width = if step % 2 == 0 { 10. } else { 15. };
        layout(&control, session, step, vec![a.clone()]);
        let desired = reader.snapshot();
        assert_eq!(desired.routing_generation, installed.routing_generation);
        assert!(installed.routing_guard.is_current());
        assert!(!installed.guard.is_current());
        let token = crate::runtime::CompletionToken {
            device,
            serial: u64::from(step),
        };
        assert!(
            reader
                .mark_frame_installed(installed.routing_generation, token)
                .is_none()
        );
        let actual = region.for_frame(token);
        assert_eq!(backing.for_frame(token).destination, installed.views[0].backdrop);
        assert_eq!(actual.destination, desired.views[0].destination);
        assert_eq!(actual.source_rect, desired.views[0].source_rect);
        assert_eq!(
            reader.mark_frame_installed(installed.routing_generation, token),
            Some(desired.generation)
        );
    }
    a.source = "replacement".into();
    layout(&control, session, 100, vec![a]);
    assert!(
        !installed.routing_guard.is_current(),
        "source changes still fence resource installation"
    );
}

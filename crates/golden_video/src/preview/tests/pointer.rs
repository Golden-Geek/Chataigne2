use super::*;
use crate::{
    plan::{OverlayPrimitive, OverlayShape},
    runtime::{CompletionToken, DeviceGeneration},
};

#[test]
fn pointer_uses_the_accepted_frame_not_newer_desired_or_rendered_geometry() {
    let (control, reader, session) = fixture();
    let mut view = view();
    view.rect = CssRect {
        x: 0.0,
        y: 0.0,
        width: 20.0,
        height: 20.0,
    };
    view.clip = CssRect {
        x: 0.0,
        y: 0.0,
        width: 30.0,
        height: 30.0,
    };
    view.overlay_capacity = 2;
    view.overlays = vec![
        OverlayPrimitive {
            hit: Some("body".into()),
            shape: OverlayShape::Quad {
                corners: [[0., 0.], [1., 0.], [1., 1.], [0., 1.]],
            },
            fill: [0.0; 4],
            stroke: [0.0; 4],
            width: 0.0,
            dash: None,
        },
        OverlayPrimitive {
            hit: Some("corner".into()),
            shape: OverlayShape::Circle {
                center: [0.5, 0.5],
                radius: 2.0,
            },
            fill: [1.; 4],
            stroke: [1.; 4],
            width: 0.0,
            dash: None,
        },
    ];
    let request = PreviewRequest::Pointer {
        session,
        view: view.id,
        source: view.source.clone(),
        point: [10., 10.],
    };
    layout(&control, session, 1, vec![view.clone()]);
    assert_eq!(
        control.handle(request.clone(), host()),
        PreviewResponse::Pointer { sample: None }
    );
    let output = crate::OutputId::new(1).unwrap();
    let placement = reader.snapshot().regions(output, |_| TextureId::new(1))[0]
        .placement
        .clone()
        .unwrap();
    let token = CompletionToken {
        device: DeviceGeneration::new(Uuid::new_v4(), 1),
        serial: 1,
    };
    placement.sample(token);
    assert_eq!(
        control.handle(request.clone(), host()),
        PreviewResponse::Pointer { sample: None },
        "rendering is not output acceptance"
    );
    assert!(
        !placement.mark_presented(crate::OutputId::new(2).unwrap(), token),
        "another consumer cannot authorize this view's input"
    );
    assert!(placement.presented().is_none());
    assert!(placement.mark_presented(output, token));
    let PreviewResponse::Pointer { sample: Some(first) } = control.handle(request.clone(), host()) else {
        panic!()
    };
    assert_eq!(first.hit.as_deref(), Some("corner"));
    assert!((first.point[0] - 0.5).abs() < 0.00001);
    view.rect.x = 8.0;
    layout(&control, session, 2, vec![view.clone()]);
    let next = CompletionToken { serial: 2, ..token };
    placement.sample(next);
    assert_eq!(
        control.handle(request.clone(), host()),
        PreviewResponse::Pointer {
            sample: Some(first.clone())
        }
    );
    assert!(
        !placement.mark_presented(output, token),
        "cannot promote a superseded sample"
    );
    assert!(placement.mark_presented(output, next));
    let PreviewResponse::Pointer { sample: Some(moved) } = control.handle(request.clone(), host()) else {
        panic!()
    };
    assert_eq!(moved.hit.as_deref(), Some("body"));
    assert!(moved.point[0] < 0.2);
    view.source = "replacement".into();
    layout(&control, session, 3, vec![view.clone()]);
    assert_eq!(
        control.handle(request, host()),
        PreviewResponse::Pointer { sample: None }
    );
    let current = PreviewRequest::Pointer {
        session,
        view: view.id,
        source: view.source,
        point: [10., 10.],
    };
    assert_eq!(
        control.handle(current, host()),
        PreviewResponse::Pointer { sample: None },
        "source change cannot inherit accepted input"
    );
}

#[test]
fn clearing_presentation_device_changes_and_invalid_input_withhold_native_hits() {
    let (control, reader, session) = fixture();
    let view = view();
    layout(&control, session, 1, vec![view.clone()]);
    let output = crate::OutputId::new(1).unwrap();
    let placement = reader.snapshot().regions(output, |_| TextureId::new(1))[0]
        .placement
        .clone()
        .unwrap();
    let token = CompletionToken {
        device: DeviceGeneration::new(Uuid::new_v4(), 1),
        serial: 1,
    };
    placement.sample(token);
    placement.mark_presented(output, token);
    let request = PreviewRequest::Pointer {
        session,
        view: view.id,
        source: view.source,
        point: [1., 1.],
    };
    reader.mark_uninstalled(reader.snapshot().generation);
    assert!(
        placement.presented().is_some(),
        "a pending replacement retains the accepted image and its input"
    );
    placement.clear_presented();
    assert_eq!(
        control.handle(request.clone(), host()),
        PreviewResponse::Pointer { sample: None }
    );
    placement.sample(CompletionToken {
        device: DeviceGeneration::new(Uuid::new_v4(), 2),
        serial: 1,
    });
    assert!(placement.presented().is_none());
    let PreviewRequest::Pointer { view, source, .. } = request else {
        unreachable!()
    };
    assert_eq!(
        control.handle(
            PreviewRequest::Pointer {
                session,
                view,
                source,
                point: [f64::NAN, 0.]
            },
            host()
        ),
        PreviewResponse::Rejected {
            code: PreviewRejection::Geometry
        }
    );
}

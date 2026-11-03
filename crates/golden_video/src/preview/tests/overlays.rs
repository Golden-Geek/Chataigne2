use super::*;
use crate::plan::{MAX_VIEW_OVERLAYS, OverlayPrimitive, OverlayShape};

fn overlay() -> OverlayPrimitive {
    OverlayPrimitive {
        shape: OverlayShape::Circle {
            center: [0.5, 0.5],
            radius: 6.8,
        },
        fill: [0.8, 0.9, 1.0, 1.0],
        stroke: [0.2, 0.4, 0.8, 1.0],
        width: 1.6,
        dash: None,
    }
}

#[test]
fn admission_keeps_host_scale_and_aperture_with_bounded_decorations() {
    let (control, reader, session) = fixture();
    let mut definition = view();
    definition.overlay_capacity = MAX_VIEW_OVERLAYS as u32;
    definition.overlays.push(overlay());
    assert!(matches!(
        layout(&control, session, 1, vec![definition.clone()]),
        PreviewResponse::Applied { .. }
    ));
    let first = reader.snapshot();
    let region = first.regions(|_| TextureId::new(1)).remove(0);
    let token = crate::runtime::CompletionToken {
        device: crate::runtime::DeviceGeneration::new(Uuid::new_v4(), 1),
        serial: 1,
    };
    let placement = region.placement.unwrap();
    let geometry = placement.sample(token);
    assert_eq!(geometry.overlay_scale, 2.0);
    assert_eq!(geometry.overlay_clip, first.views[0].backdrop);
    assert_eq!(geometry.overlays.as_ref(), &[overlay()]);
    definition.overlays.clear();
    layout(&control, session, 2, vec![definition]);
    assert!(
        first.routing_guard.is_current(),
        "presentation changes must not recreate image resources"
    );
    assert!(!placement.sample(token).overlays.is_empty());
    assert!(
        placement
            .sample(crate::runtime::CompletionToken { serial: 2, ..token })
            .overlays
            .is_empty()
    );
}

#[test]
fn malformed_or_excessive_overlay_metadata_withdraws_the_rejected_layout() {
    let (control, reader, session) = fixture();
    layout(&control, session, 1, vec![view()]);
    let before = reader.snapshot();
    let mut excessive = view();
    excessive.overlay_capacity = MAX_VIEW_OVERLAYS as u32;
    excessive.overlays = vec![overlay(); MAX_VIEW_OVERLAYS + 1];
    assert_eq!(
        layout(&control, session, 2, vec![excessive]),
        PreviewResponse::Rejected {
            code: PreviewRejection::Geometry
        }
    );
    let mut invalid = view();
    invalid.overlay_capacity = 1;
    invalid.overlays.push(OverlayPrimitive {
        fill: [f32::NAN; 4],
        ..overlay()
    });
    assert_eq!(
        layout(&control, session, 3, vec![invalid]),
        PreviewResponse::Rejected {
            code: PreviewRejection::Geometry
        }
    );
    assert!(!before.guard.is_current());
    assert!(reader.snapshot().views.is_empty());
}

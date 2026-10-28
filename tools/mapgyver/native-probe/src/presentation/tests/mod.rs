use super::*;

fn layout(left: f64, width: f64) -> ProbeLayout {
    ProbeLayout {
        session: 1,
        revision: 1,
        device_pixel_ratio: 2.0,
        rects: vec![CssRegion {
            bounds: CssRect {
                left,
                top: -10.0,
                width,
                height: 30.0,
            },
            clip: CssRect {
                left: 0.0,
                top: 0.0,
                width: 20.0,
                height: 20.0,
            },
        }],
    }
}

#[test]
fn document_replacement_rejects_late_layouts_and_accepts_fresh_low_revisions() {
    let mut status = ProbeStatus::default();
    assert_eq!(status.begin_document().unwrap(), 1);
    let mut old = layout(1.0, 20.0);
    old.revision = 100;
    status.apply_layout(&old, 1.25).unwrap();
    assert_eq!(status.layout_revision, 100);
    assert_eq!(status.begin_document().unwrap(), 2);
    assert!(status.rects.is_empty());
    assert!(status.apply_layout(&old, 1.25).is_err());
    assert_eq!(status.layout_revision, 0);
    let mut fresh = layout(2.0, 30.0);
    fresh.session = 2;
    status.apply_layout(&fresh, 1.0).unwrap();
    assert_eq!(status.layout_revision, 1);
    assert_eq!(status.rects[0].bounds.left, 2);
    assert!(status.apply_layout(&old, 1.25).is_err());
    assert_eq!(status.rects[0].bounds.left, 2);
}

#[test]
fn clipped_preview_keeps_original_sampling_viewport_at_mixed_scales() {
    for scale in [1.0, 1.25, 1.5, 2.0] {
        let region = layout(-20.0, 100.0).physical_rects(scale).unwrap()[0];
        assert_eq!(region.bounds.left, (-20.0 * scale) as i32);
        assert_eq!(region.bounds.width, (100.0 * scale) as u32);
        assert_eq!(region.clip.left, 0);
        assert_eq!(region.clip.width, (20.0 * scale) as u32);
        assert!(
            region.bounds.width > region.clip.width,
            "clipping must not stretch the source into the visible part"
        );
    }
}

#[test]
fn adjacent_fractional_css_edges_quantize_to_one_shared_physical_edge() {
    for scale in [1.25, 1.5, 2.0] {
        let left = layout(0.2, 10.3).physical_rects(scale).unwrap()[0].bounds;
        let right = layout(10.5, 10.3).physical_rects(scale).unwrap()[0].bounds;
        assert_eq!(left.left + left.width as i32, right.left);
    }
}

#[test]
fn invalid_or_unbounded_layouts_are_rejected_before_publication() {
    for width in [-1.0, f64::INFINITY, f64::NAN, 100_001.0] {
        assert!(layout(0.0, width).physical_rects(1.0).is_err());
    }
    assert!(layout(0.0, 10.0).physical_rects(f64::NAN).is_err());
    let mut too_many = layout(0.0, 10.0);
    too_many.rects = vec![too_many.rects[0].clone(); 3];
    assert!(too_many.physical_rects(1.0).is_err());
    too_many.rects.clear();
    assert!(
        too_many.physical_rects(1.0).unwrap().is_empty(),
        "hidden/unmounted previews publish an empty set"
    );
}

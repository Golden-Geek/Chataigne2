use super::*;

const EXTENT: UiBrowserPreviewExtent = UiBrowserPreviewExtent {
    width: 1080,
    height: 1920,
};

#[test]
fn preview_demands_share_latest_frame_and_rotate_on_expiry_or_project_change() {
    let previews = BrowserPreviews::default();
    assert!(previews.request(1, "channel", High).unwrap().is_none());
    let original = previews.active(1).pop().unwrap();
    original.publish_frame(EXTENT, |_| Ok::<_, ()>(vec![2])).unwrap();
    let image = previews.request(1, "channel", High).unwrap().unwrap();
    assert_eq!(&*image.jpeg, &[2]);
    assert_eq!(image.extent, EXTENT);
    assert_eq!(previews.active(1).len(), 1);

    let late = Instant::now() + DEMAND_LEASE;
    previews.0.lock().unwrap().expire(1, late);
    assert!(previews.active(1).is_empty());
    assert!(previews.request(1, "channel", High).unwrap().is_none());
    assert_ne!(original.id(), previews.active(1)[0].id());

    assert!(previews.request(2, "channel", High).unwrap().is_none());
    assert!(previews.request(1, "channel", High).unwrap().is_none());
    assert!(previews.active(1).is_empty());
    assert_eq!(previews.active(2).len(), 1);
}

#[test]
fn preview_demands_have_a_fixed_admission_bound() {
    let previews = BrowserPreviews::default();
    assert!(previews.request(1, &"a".repeat(MAX_SOURCE_BYTES + 1), Low).is_err());
    for index in 0..BrowserPreviews::SOURCE_LIMIT + 1 {
        assert!(previews.request(1, &index.to_string(), Low).unwrap().is_none());
    }
    assert_eq!(previews.active(1).len(), BrowserPreviews::SOURCE_LIMIT);
}

#[test]
fn two_browser_qualities_share_one_source_and_only_encode_due_active_variants() {
    let previews = BrowserPreviews::default();
    assert!(previews.request(1, "channel", Low).unwrap().is_none());
    let stream = previews.active(1).pop().unwrap();
    let now = Instant::now();
    let mut encoded_widths = Vec::new();
    let mut publish = || {
        stream
            .publish_frame_at(now, EXTENT, |variant| {
                encoded_widths.push(variant.width);
                Ok::<_, ()>(vec![variant.jpeg_quality])
            })
            .unwrap()
    };
    publish();
    assert!(previews.request(1, "channel", High).unwrap().is_none());
    publish();
    let low = previews.request(1, "channel", Low).unwrap().unwrap();
    let high = previews.request(1, "channel", High).unwrap().unwrap();
    {
        let mut variants = stream.variants.lock().unwrap();
        variants[0].encoded_at = None;
    }
    publish();
    assert_eq!(previews.request(1, "channel", Low).unwrap().unwrap().tag, low.tag);
    assert_ne!(low.tag, high.tag);
    assert_ne!(low.jpeg, high.jpeg);
    assert_eq!(encoded_widths, [320, 640, 320]);
    assert_eq!(previews.active(1).len(), 1);
}

use super::*;

#[test]
fn preview_demands_share_latest_frame_and_rotate_on_expiry_or_project_change() {
    let previews = BrowserPreviews::default();
    assert!(previews.request(1, b"channel").unwrap().is_none());
    let original = previews.active(1).pop().unwrap();
    original.publish_jpeg(vec![1]);
    original.publish_jpeg(vec![2]);
    assert_eq!(&*previews.request(1, b"channel").unwrap().unwrap().jpeg, &[2]);
    assert_eq!(previews.active(1).len(), 1);

    let late = Instant::now() + DEMAND_LEASE;
    previews.0.lock().unwrap().expire(1, late);
    assert!(previews.active(1).is_empty());
    assert!(previews.request(1, b"channel").unwrap().is_none());
    assert_ne!(original.id(), previews.active(1)[0].id());

    assert!(previews.request(2, b"channel").unwrap().is_none());
    assert!(previews.request(1, b"channel").unwrap().is_none());
    assert!(previews.active(1).is_empty());
    assert_eq!(previews.active(2).len(), 1);
}

#[test]
fn preview_demands_have_a_fixed_admission_bound() {
    let previews = BrowserPreviews::default();
    assert!(previews.request(1, &[b'a'; MAX_SOURCE_BYTES + 1]).is_err());
    for index in 0..MAX_SOURCES + 1 {
        assert!(previews.request(1, index.to_string().as_bytes()).unwrap().is_none());
    }
    assert_eq!(previews.active(1).len(), MAX_SOURCES);
}

use super::*;

#[test]
fn decoded_publication_metadata_stays_with_its_exact_image_and_is_cleared_on_plain_replacement() {
    use crate::scheduling::FrameWindow;
    use golden_time::{PlaybackRate, ShowPosition, SystemClock, TimeAuthority};
    use std::time::Instant;
    let mut authority =
        TimeAuthority::new(SystemClock::new(), ShowPosition::from_nanos(0), PlaybackRate::NORMAL).unwrap();
    let window = FrameWindow::new(
        authority.sample().unwrap().epoch(),
        ShowPosition::from_nanos(20),
        None,
        ShowPosition::from_nanos(10),
    )
    .unwrap();
    let stream = ImageStream::new(descriptor(2, 1));
    let before = Instant::now();
    stream
        .replace_frame(ImageData::new(descriptor(2, 1), vec![17; 8]).unwrap(), window)
        .unwrap();
    let observed = stream.sample().unwrap();
    let frame = observed.frame.unwrap();
    assert_eq!(frame.window, window);
    assert!(frame.published_at >= before && frame.published_at <= Instant::now());
    assert!(
        stream
            .replace_frame(ImageData::new(descriptor(1, 1), vec![0; 4]).unwrap(), window)
            .is_err()
    );
    let held = stream.clone().sample().unwrap();
    assert_eq!(held.sequence, observed.sequence);
    assert_eq!(held.frame.unwrap().published_at, frame.published_at);
    assert_eq!(held.image.unwrap().pixels(), &[17; 8]);
    stream.publish(None).unwrap();
    assert!(stream.sample().unwrap().frame.is_none());
    assert_eq!(observed.image.unwrap().pixels(), &[17; 8]);
    stream
        .replace_frame(ImageData::new(descriptor(2, 1), vec![18; 8]).unwrap(), window)
        .unwrap();
    stream
        .publish(Some(ImageData::new(descriptor(2, 1), vec![19; 8]).unwrap()))
        .unwrap();
    let plain = stream.sample().unwrap();
    assert!(plain.frame.is_none());
    assert_eq!(plain.image.unwrap().pixels(), &[19; 8]);
}

#[test]
fn latest_frames_replace_without_queues_and_keep_storage_admission_through_last_reader() {
    let stream = ImageStream::new(descriptor(2, 1));
    let lease = std::sync::Arc::new(());
    let weak = std::sync::Arc::downgrade(&lease);
    stream
        .publish(Some(
            ImageData::new(descriptor(2, 1), vec![17; 8])
                .unwrap()
                .retain(lease.clone()),
        ))
        .unwrap();
    let observed = stream.sample().unwrap();
    drop(lease);
    let sequence = observed.sequence;
    assert!(
        stream
            .publish(Some(ImageData::new(descriptor(1, 1), vec![0; 4]).unwrap()))
            .is_err()
    );
    assert_eq!(stream.sample().unwrap().sequence, sequence);
    stream.publish(None).unwrap();
    assert!(stream.sample().unwrap().image.is_none());
    assert!(
        weak.upgrade().is_some(),
        "sample retains its own exact storage reservation"
    );
    assert_eq!(observed.image.as_ref().unwrap().pixels(), &[17; 8]);
    drop(observed);
    assert!(weak.upgrade().is_none());
    for value in 0..100 {
        stream
            .publish(Some(ImageData::new(descriptor(2, 1), vec![value; 8]).unwrap()))
            .unwrap();
    }
    assert_eq!(stream.sample().unwrap().image.unwrap().pixels(), &[99; 8]);
}

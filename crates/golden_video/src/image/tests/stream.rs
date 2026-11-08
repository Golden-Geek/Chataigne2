use super::*;

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

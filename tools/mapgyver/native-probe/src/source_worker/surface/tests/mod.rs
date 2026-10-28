use super::*;

#[test]
fn only_exact_s_ok_grants_surface_ownership() {
    assert!(acquired(0).unwrap());
    assert!(!acquired(258).unwrap());
    assert!(acquired(128).is_err());
    assert!(acquired(-1).is_err());
}

#[test]
#[ignore = "creates two Intel hardware devices; finite GPU-only sharing and diagnostic pixel checks"]
fn native_bridge_holds_three_readers_without_reusing_their_surfaces() {
    use std::time::{Duration, Instant};
    let producer = Gpu::new(0).unwrap();
    let consumer = Gpu::new(0).unwrap();
    let shared = Arc::new(Shared::default());
    let mut slots = Vec::new();
    let mut frames = Vec::new();
    for _ in 0..super::super::CAPACITY {
        let slot = Slot::new(&producer, [32, 16]).unwrap();
        assert!(slot.acquire_free().unwrap());
        producer.clear(&slot.texture, [1.0, 0.0, 0.0, 1.0]).unwrap();
        // SAFETY: this producer acquired the new slot and submitted its clear before handoff.
        unsafe {
            producer.context.Flush();
            slot.mutex.ReleaseSync(1).unwrap();
        }
        let deadline = Instant::now() + Duration::from_secs(2);
        let frame = loop {
            if let Some(frame) = Frame::open(
                &consumer,
                Publication {
                    surface: slot.surface.clone(),
                    batch: 1,
                },
                shared.clone(),
            )
            .unwrap()
            {
                break frame;
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(1));
        };
        assert_eq!(consumer.read_marker(&frame.texture).unwrap(), [0, 0, 255, 255]);
        frames.push(frame);
        slots.push(slot);
    }
    for _ in 0..100 {
        assert!(slots.iter().all(|slot| !slot.acquire_free().unwrap()));
    }
    frames.pop();
    assert!(slots[2].acquire_free().unwrap());
    producer.clear(&slots[2].texture, [0.0, 0.0, 1.0, 1.0]).unwrap();
    // SAFETY: the only reader of this slot dropped; the producer reacquired ownership above.
    unsafe {
        producer.context.Flush();
        slots[2].mutex.ReleaseSync(1).unwrap();
    }
    let deadline = Instant::now() + Duration::from_secs(2);
    let replacement = loop {
        if let Some(frame) = Frame::open(
            &consumer,
            Publication {
                surface: slots[2].surface.clone(),
                batch: 2,
            },
            shared.clone(),
        )
        .unwrap()
        {
            break frame;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    };
    assert_eq!(consumer.read_marker(&replacement.texture).unwrap(), [255, 0, 0, 255]);
    assert_eq!(replacement.batch, 2);
    assert!(frames.iter().all(|frame| frame.batch == 1));
    drop(replacement);
    drop(frames);
    consumer.retire().unwrap();
    assert_eq!(shared.unlock_failures.load(Ordering::Relaxed), 0);
    drop(slots);
    producer.retire().unwrap();
    assert_eq!(producer.allocations.snapshot().live_textures, 0);
    assert_eq!(producer.allocations.snapshot().retiring_textures, 0);
}

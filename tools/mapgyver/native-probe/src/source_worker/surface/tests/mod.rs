use super::*;
use crate::source_worker::{
    Shared,
    imports::{Frame, Imports},
};
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

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
    let producer = Gpu::new(0).unwrap();
    let consumer = Gpu::new(0).unwrap();
    let mut imports = Imports::new(&consumer, super::super::CAPACITY).unwrap();
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
        let frame = receive(&mut imports, &slot, 1, &shared);
        assert_eq!(consumer.read_marker(frame.texture()).unwrap(), [0, 0, 255, 255]);
        assert_eq!(Arc::strong_count(&slot.surface), 2);
        frames.push(frame);
        slots.push(slot);
    }
    assert_eq!(imports.counts().opens, 3);
    assert_eq!(imports.counts().cache_misses, 3);
    assert_eq!(imports.cached(), 3);
    for _ in 0..100 {
        assert!(slots.iter().all(|slot| !slot.acquire_free().unwrap()));
    }
    frames.pop();
    // Cached renderer interfaces retain the GPU resource, never producer reader ownership.
    assert_eq!(Arc::strong_count(&slots[2].surface), 1);
    assert!(slots[2].acquire_free().unwrap());
    producer.clear(&slots[2].texture, [0.0, 0.0, 1.0, 1.0]).unwrap();
    // SAFETY: the only reader of this slot dropped; the producer reacquired ownership above.
    unsafe {
        producer.context.Flush();
        slots[2].mutex.ReleaseSync(1).unwrap();
    }
    let replacement = receive(&mut imports, &slots[2], 2, &shared);
    assert_eq!(consumer.read_marker(replacement.texture()).unwrap(), [255, 0, 0, 255]);
    assert_eq!(replacement.batch, 2);
    assert!(frames.iter().all(|frame| frame.batch == 1));
    assert_eq!(imports.counts().opens, 3);
    assert!(imports.counts().cache_hits >= 1);
    drop(replacement);
    consumer.retire().unwrap();
    assert_eq!(Arc::strong_count(&slots[2].surface), 1);
    assert!(slots[2].acquire_free().unwrap());
    // SAFETY: ownership was acquired above; close it before replacing the allocation.
    unsafe { slots[2].mutex.ReleaseSync(0).unwrap() };
    slots[2] = Slot::new(&producer, [48, 24]).unwrap();
    assert!(slots[2].acquire_free().unwrap());
    producer.clear(&slots[2].texture, [0.0, 1.0, 0.0, 1.0]).unwrap();
    // SAFETY: this new allocation has producer ownership and its clear precedes handoff.
    unsafe {
        producer.context.Flush();
        slots[2].mutex.ReleaseSync(1).unwrap();
    }
    let resized = receive(&mut imports, &slots[2], 3, &shared);
    assert_eq!(resized.surface.extent, [48, 24]);
    assert_eq!(consumer.read_marker(resized.texture()).unwrap(), [0, 255, 0, 255]);
    assert_eq!(imports.counts().opens, 4);
    assert_eq!(imports.counts().cache_misses, 4);
    assert!(imports.cached() <= super::super::CAPACITY);
    assert!(!slots[2].acquire_free().unwrap());
    drop(resized);
    drop(frames);
    consumer.retire().unwrap();
    imports.clear(false);
    assert_eq!(imports.cached(), 0);
    let report = imports.finish();
    assert_eq!(report.capacity, super::super::CAPACITY);
    assert_eq!(report.peak_cached, 3);
    assert_eq!(report.cached_after_clear, 0);
    assert_eq!(report.live_imports_after_clear, 0);
    assert_eq!(report.all.opens, 4);
    assert_eq!(report.all.cache_misses, 4);
    assert_eq!(report.all.pruned, 1);
    assert_eq!(shared.unlock_failures.load(Ordering::Relaxed), 0);
    drop(slots);
    producer.retire().unwrap();
    assert_eq!(producer.allocations.snapshot().live_textures, 0);
    assert_eq!(producer.allocations.snapshot().retiring_textures, 0);
}

#[test]
#[ignore = "creates two Intel hardware devices; finite keyed-mutex timeout and diagnostic pixel checks"]
fn native_bridge_caches_a_timeout_without_releasing_an_unacquired_key() {
    let producer = Gpu::new(0).unwrap();
    let consumer = Gpu::new(0).unwrap();
    let mut imports = Imports::new(&consumer, super::super::CAPACITY).unwrap();
    let shared = Arc::new(Shared::default());
    let slot = Slot::new(&producer, [32, 16]).unwrap();
    assert!(slot.acquire_free().unwrap());
    producer.clear(&slot.texture, [0.0, 1.0, 0.0, 1.0]).unwrap();
    // The producer still owns the mutex. Importing the allocation succeeds, but the
    // consumer must time out on key 1 without constructing or releasing a reader.
    assert!(
        imports
            .open(
                Publication {
                    surface: slot.surface.clone(),
                    batch: 1,
                },
                shared.clone(),
                true,
            )
            .unwrap()
            .is_none()
    );
    assert_eq!(imports.counts().opens, 1);
    assert_eq!(imports.counts().cache_misses, 1);
    assert_eq!(imports.counts().cache_hits, 0);
    assert_eq!(imports.counts().acquire_timeouts, 1);
    assert_eq!(imports.cached(), 1);
    assert_eq!(Arc::strong_count(&slot.surface), 1);
    assert_eq!(shared.unlock_failures.load(Ordering::Relaxed), 0);
    // SAFETY: the producer's original acquisition remains held through the failed poll.
    unsafe {
        producer.context.Flush();
        slot.mutex.ReleaseSync(1).unwrap();
    }
    let frame = receive(&mut imports, &slot, 1, &shared);
    assert_eq!(consumer.read_marker(frame.texture()).unwrap(), [0, 255, 0, 255]);
    assert_eq!(imports.counts().opens, 1);
    assert_eq!(imports.counts().cache_misses, 1);
    // All attempts after the initial miss hit the same cached import. There is one
    // successful acquisition, so hit count equals timeout count even if handoff retries.
    assert_eq!(imports.counts().cache_hits, imports.counts().acquire_timeouts);
    assert_eq!(Arc::strong_count(&slot.surface), 2);
    drop(frame);
    consumer.retire().unwrap();
    assert_eq!(Arc::strong_count(&slot.surface), 1);
    assert!(slot.acquire_free().unwrap());
    // SAFETY: the completed reader released key 0, acquired immediately above.
    unsafe { slot.mutex.ReleaseSync(0).unwrap() };
    imports.clear(false);
    let report = imports.finish();
    assert_eq!(report.cached_after_clear, 0);
    assert_eq!(report.live_imports_after_clear, 0);
    assert_eq!(report.all.opens, 1);
    assert_eq!(report.all.evictions, 0);
    assert_eq!(shared.unlock_failures.load(Ordering::Relaxed), 0);
    drop(slot);
    producer.retire().unwrap();
    assert_eq!(producer.allocations.snapshot().live_textures, 0);
    assert_eq!(producer.allocations.snapshot().retiring_textures, 0);
}

fn receive(imports: &mut Imports, slot: &Slot, batch: u64, shared: &Arc<Shared>) -> Frame {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if let Some(frame) = imports
            .open(
                Publication {
                    surface: slot.surface.clone(),
                    batch,
                },
                shared.clone(),
                true,
            )
            .unwrap()
        {
            return frame;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
}

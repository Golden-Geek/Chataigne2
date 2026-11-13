use super::*;

#[test]
fn cpu_admission_survives_cancelled_owner_and_last_image_observer_with_stable_identity() {
    let files = Files::new(Path::new("."), None);
    let observer = files.retirement();
    let initial = observer.cpu_admission();
    assert_eq!(initial.retained_bytes, 0);
    let lease = Arc::new(files.budget.reserve(64 * 48).unwrap());
    let descriptor = golden_video::TextureDescriptor {
        extent: golden_video::Extent2D::new(1, 1).unwrap(),
        format: golden_video::PixelFormat::Rgba8Unorm,
        color_encoding: golden_video::ColorEncoding::Srgb,
        alpha_mode: golden_video::AlphaMode::Straight,
    };
    let image = golden_video::image::ImageData::new(descriptor, vec![127; 4])
        .unwrap()
        .retain(lease.clone());
    let held = observer.cpu_admission();
    assert_eq!(held.id, initial.id);
    assert_eq!(held.limit_bytes, MAX_BYTES);
    assert_eq!(held.allocations, 1);
    assert_eq!(held.retained_bytes, 64 * 48 * 40 + 16 * 1024 * 1024);
    assert_eq!(held.peak_bytes, held.retained_bytes);
    drop(lease);
    drop(files);
    assert_eq!(observer.cpu_admission(), held);
    assert!(!observer.wait(Duration::ZERO));
    drop(image);
    assert!(observer.wait(Duration::from_millis(100)));
    let retired = observer.cpu_admission();
    assert_eq!(retired.id, initial.id);
    assert_eq!(retired.retained_bytes, 0);
    assert_eq!(retired.allocations, 0);
    assert_eq!(retired.peak_bytes, held.peak_bytes);
}

#[test]
fn cpu_capacity_zero_and_overflow_rejections_leave_the_snapshot_unchanged() {
    let budget = Arc::new(Budget::default());
    let initial = budget.snapshot();
    assert!(budget.reserve(u64::MAX).is_err());
    assert!(budget.reserve_bytes(0).is_err());
    assert_eq!(budget.snapshot(), initial);
    let lease = budget.reserve_bytes(MAX_BYTES).unwrap();
    let full = budget.snapshot();
    assert!(budget.reserve_bytes(1).is_err());
    assert_eq!(budget.snapshot(), full);
    drop(lease);
    assert_eq!(budget.snapshot().retained_bytes, 0);
    assert_eq!(budget.snapshot().allocations, 0);
    assert_eq!(budget.snapshot().peak_bytes, MAX_BYTES);
}

#[test]
fn concurrent_cpu_admission_and_retirement_keep_group_bytes_and_peak_coherent() {
    let budget = Arc::new(Budget::default());
    std::thread::scope(|scope| {
        for _ in 0..4 {
            let budget = budget.clone();
            scope.spawn(move || {
                for _ in 0..1000 {
                    let lease = budget.reserve_bytes(16).unwrap();
                    let snapshot = budget.snapshot();
                    assert_eq!(snapshot.retained_bytes, snapshot.allocations * 16);
                    assert!(snapshot.peak_bytes >= snapshot.retained_bytes);
                    assert!(snapshot.peak_bytes <= 64);
                    drop(lease);
                }
            });
        }
    });
    assert_eq!(budget.snapshot().retained_bytes, 0);
    assert_eq!(budget.snapshot().allocations, 0);
}

use super::*;

#[test]
fn returned_pixel_storage_keeps_its_caller_budget_until_the_snapshot_is_dropped() {
    let budget = PayloadPool::new("probe-cpu-snapshot", 1, 16).unwrap();
    let descriptor = TextureDescriptor {
        extent: Extent2D::new(4, 1).unwrap(),
        format: PixelFormat::Bgra8Unorm,
        color_encoding: ColorEncoding::Srgb,
        alpha_mode: AlphaMode::Straight,
    };
    let image = ProbePixels {
        extent: descriptor.extent,
        pixels: vec![[1, 2, 3, 4]; 4],
        _reservation: budget.try_reserve(16).unwrap(),
    };
    assert_eq!(image.extent(), descriptor.extent);
    assert_eq!(image.pixels(), &[[1, 2, 3, 4]; 4]);
    assert!(budget.try_reserve(1).is_err());
    drop(image);
    assert!(budget.wait_for_idle(Duration::from_secs(1)));
    assert!(budget.try_reserve(16).is_ok());
}

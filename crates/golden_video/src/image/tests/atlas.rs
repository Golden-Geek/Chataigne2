use super::*;

#[test]
fn packed_assets_preserve_exact_pixels_transparent_guards_and_checked_regions() {
    let red = ImageData::new(descriptor(2, 1), vec![255, 0, 0, 128, 10, 20, 30, 255]).unwrap();
    let blue = ImageData::new(descriptor(1, 2), vec![0, 0, 255, 255, 40, 50, 60, 0]).unwrap();
    let atlas = ImageAtlas::pack(&[&red, &blue]).unwrap();
    let image = atlas.image();
    assert_eq!(image.descriptor(), descriptor(4, 7));
    let mut expected = vec![0; 4 * 7 * 4];
    expected[20..28].copy_from_slice(red.pixels());
    expected[68..72].copy_from_slice(&blue.pixels()[..4]);
    expected[84..88].copy_from_slice(&blue.pixels()[4..]);
    assert_eq!(image.pixels(), expected);
    assert_eq!(
        atlas.region(0),
        Some(crate::plan::UnitRect {
            x: 0.25,
            y: 1. / 7.,
            width: 0.5,
            height: 1. / 7.
        })
    );
    assert_eq!(
        atlas.region(1),
        Some(crate::plan::UnitRect {
            x: 0.25,
            y: 4. / 7.,
            width: 0.25,
            height: 2. / 7.
        })
    );
    assert!(atlas.region(2).is_none());
    assert!(ImageAtlas::pack(&[]).is_err());
    assert!(ImageAtlas::pack(&vec![&red; 33]).is_err());
    let invalid = ImageData::new(
        TextureDescriptor {
            alpha_mode: AlphaMode::Opaque,
            ..descriptor(1, 1)
        },
        vec![0; 4],
    )
    .unwrap();
    assert!(ImageAtlas::pack(&[&invalid]).is_err());
    assert!(FontAtlas::rasterize(&[], 1).is_err());
    assert!(FontAtlas::rasterize(&[], 257).is_err());
    assert!(FontAtlas::rasterize(&[0; 16], 16).is_err());
}

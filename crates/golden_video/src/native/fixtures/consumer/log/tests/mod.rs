use super::*;

#[test]
fn tiled_log_pages_keep_exact_offsets_and_explicit_padding_within_native_bounds() {
    for capacity in [1, 16384, 16385, 32768, 32769, 65536] {
        let layout = Layout::new(capacity).unwrap();
        assert!(layout.width <= 12 && layout.height <= 16384);
        assert_eq!(layout.origin(capacity), None);
        assert!(
            layout
                .origin(capacity - 1)
                .is_some_and(|[x, y]| x + 3 <= layout.width && y < layout.height)
        );
        let pitch = layout.width * 16 + 256;
        for row in [0, capacity - 1] {
            let [x, y] = layout.origin(row).unwrap();
            assert_eq!(layout.offset(row, pitch), Some((y * pitch + x * 16) as usize));
        }
    }
    let layout = Layout::new(16385).unwrap();
    assert_eq!(layout.origin(16383), Some([0, 16383]));
    assert_eq!(layout.origin(16384), Some([3, 0]));
    assert_eq!(layout.bytes(), 6 * 16384 * 16, "unused final-page rows remain charged");
    assert_eq!(ROW_BYTES, 48);
    assert!(Layout::new(0).is_err());
    assert!(Layout::new(65537).is_err());
}

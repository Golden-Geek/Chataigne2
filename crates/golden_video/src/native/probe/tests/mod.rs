use super::*;

#[test]
fn sender_metadata_must_have_supported_bounded_storage_before_graphics_allocation() {
    let base = Endpoint {
        handle: 1,
        width: 4,
        height: 4,
        format: 87,
    };
    assert_eq!(base.descriptor().unwrap().format, PixelFormat::Bgra8Unorm);
    for (storage, format, bytes) in [
        (10, PixelFormat::Rgba16Float, 128),
        (2, PixelFormat::Rgba32Float, 256),
        (11, PixelFormat::Rgba16Unorm, 128),
        (13, PixelFormat::Rgba16Snorm, 128),
        (31, PixelFormat::Rgba8Snorm, 64),
        (24, PixelFormat::Rgb10A2Unorm, 64),
        (88, PixelFormat::Bgrx8Unorm, 64),
    ] {
        let endpoint = Endpoint {
            format: storage,
            ..base
        };
        let descriptor = endpoint.descriptor().unwrap();
        assert_eq!(descriptor.format, format);
        assert_eq!(
            descriptor.color_encoding,
            if storage == 88 {
                ColorEncoding::Srgb
            } else {
                ColorEncoding::Linear
            }
        );
        let insufficient = PayloadPool::new("float-insufficient", 1, bytes - 1).unwrap();
        assert!(
            endpoint.reserve(&insufficient).is_err(),
            "full float storage must be admitted before allocation"
        );
        let exact = PayloadPool::new("float-exact", 1, bytes).unwrap();
        assert!(endpoint.reserve(&exact).is_ok());
    }
    assert_eq!(
        Endpoint { format: 28, ..base }.descriptor().unwrap().format,
        PixelFormat::Rgba8Unorm
    );
    for (storage, format) in [
        (29, PixelFormat::Rgba8Unorm),
        (91, PixelFormat::Bgra8Unorm),
        (93, PixelFormat::Bgrx8Unorm),
    ] {
        let endpoint = Endpoint {
            format: storage,
            ..base
        };
        let descriptor = endpoint.descriptor().unwrap();
        assert_eq!(descriptor.format, format);
        assert_eq!(descriptor.color_encoding, ColorEncoding::Srgb);
        assert_eq!(
            descriptor.alpha_mode,
            if storage == 93 {
                AlphaMode::Opaque
            } else {
                AlphaMode::Straight
            }
        );
        let exact = PayloadPool::new("srgb-exact", 1, 64).unwrap();
        assert!(endpoint.reserve(&exact).is_ok());
    }
    for endpoint in [
        Endpoint { width: 0, ..base },
        Endpoint { height: 0, ..base },
        Endpoint {
            width: u32::MAX,
            height: u32::MAX,
            ..base
        },
        Endpoint { format: 0, ..base },
    ] {
        assert!(endpoint.descriptor().is_err());
    }
    for (registered, alpha) in [(21, AlphaMode::Straight), (22, AlphaMode::Opaque)] {
        let endpoint = Endpoint {
            format: registered,
            ..base
        };
        let actual = endpoint.descriptor().unwrap();
        assert_eq!(actual.format, PixelFormat::Bgra8Unorm);
        assert_eq!(actual.color_encoding, ColorEncoding::Srgb);
        assert_eq!(actual.alpha_mode, alpha);
        let exact = PayloadPool::new("legacy-exact", 1, 64).unwrap();
        assert!(endpoint.reserve(&exact).is_ok());
    }
}

#[test]
fn probe_storage_is_reserved_before_allocation_and_shared_until_retirement() {
    let pool = PayloadPool::new("probe-budget-test", 2, 64).unwrap();
    let endpoint = Endpoint {
        handle: 1,
        width: 4,
        height: 4,
        format: 87,
    };
    let (_, reservation) = endpoint.reserve(&pool).unwrap();
    assert!(endpoint.reserve(&pool).is_err());
    drop(reservation);
    assert!(endpoint.reserve(&pool).is_ok());
    assert!(
        Endpoint {
            width: u32::MAX,
            ..endpoint
        }
        .reserve(&pool)
        .is_err()
    );
}

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
    assert_eq!(
        Endpoint { format: 28, ..base }.descriptor().unwrap().format,
        PixelFormat::Rgba8Unorm
    );
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

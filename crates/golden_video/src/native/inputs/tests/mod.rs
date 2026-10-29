use std::os::windows::io::{FromRawHandle, OwnedHandle};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use uuid::Uuid;
use windows::Win32::System::Threading::CreateEventW;
use windows::core::HRESULT;

use super::super::super::{AlphaMode, ColorEncoding, Extent2D, PixelFormat};
use super::shared::Publication;
use super::surface::{Surface, acquired};
use super::*;
use crate::runtime::DeviceGeneration;

#[test]
fn keyed_mutex_positive_timeout_and_abandoned_codes_are_not_success() {
    assert!(acquired(HRESULT(0)).unwrap());
    assert!(!acquired(HRESULT(258)).unwrap());
    assert!(acquired(HRESULT(128)).is_err());
    assert!(acquired(HRESULT(0x8000_4005_u32 as i32)).is_err());
}

#[test]
fn publication_coalescing_cannot_revive_a_disabled_or_retiring_input() {
    let descriptor = TextureDescriptor {
        extent: Extent2D::new(1, 1).unwrap(),
        format: PixelFormat::Rgba8Unorm,
        color_encoding: ColorEncoding::Linear,
        alpha_mode: AlphaMode::Premultiplied,
    };
    let status = Arc::new(Mutex::new(SourceSnapshot {
        device: DeviceGeneration::new(Uuid::new_v4(), 0),
        texture: TextureId::new(1).unwrap(),
        descriptor,
        observed_descriptor: None,
        state: SourceState::Idle,
        generation: 0,
        pool_generation: 0,
        bridge_slots: 0,
        sdk_calls: 0,
        publications: 0,
        sequence: 0,
        bridge_busy: 0,
        imports: 0,
        renderer_copies: 0,
        hresult: None,
    }));
    let shared = Arc::new(Shared {
        latest: Mutex::new(None),
        status,
        active: AtomicBool::new(false),
        demands: std::sync::atomic::AtomicUsize::new(0),
        quiescing: AtomicBool::new(false),
        retiring: AtomicBool::new(false),
        released: AtomicBool::new(false),
        unlock_failed: AtomicBool::new(false),
        shutdown: AtomicBool::new(false),
        notify: None,
        worker_notify: Mutex::new(None),
    });
    // This metadata-only fixture uses an owned kernel event; no GPU import is attempted.
    let handle = unsafe { CreateEventW(None, true, false, None) }.unwrap();
    let surface = Arc::new(Surface {
        handle: unsafe { OwnedHandle::from_raw_handle(handle.0) },
        index: 0,
        descriptor,
    });
    let publication = |sequence| Publication {
        surface: surface.clone(),
        sequence,
    };
    assert!(!shared.publish(publication(1)));
    let first = shared.acquire_demand();
    assert!(shared.publish(publication(2)));
    assert!(shared.publish(publication(3)));
    assert_eq!(shared.latest.lock().unwrap().as_ref().unwrap().sequence, 3);
    drop(first);
    assert!(shared.latest.lock().unwrap().is_none());
    assert!(!shared.publish(publication(4)));
    let second = shared.acquire_demand();
    let third = shared.acquire_demand();
    shared.retiring.store(true, Ordering::Release);
    assert!(!shared.publish(publication(5)));
    shared.retiring.store(false, Ordering::Release);
    shared.shutdown.store(true, Ordering::Release);
    assert!(!shared.publish(publication(6)));
    shared.quiesce();
    drop(second);
    assert_eq!(shared.demands.load(Ordering::Acquire), 1);
    assert!(!shared.active.load(Ordering::Acquire));
    drop(third);
    assert_eq!(shared.demands.load(Ordering::Acquire), 0);
    assert_eq!(shared.status.lock().unwrap().publications, 2);
}

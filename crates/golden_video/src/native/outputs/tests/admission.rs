use std::{
    cell::Cell,
    os::windows::io::{FromRawHandle, OwnedHandle},
};

use windows::{
    Win32::{
        Foundation::HANDLE,
        Graphics::Dxgi::DXGI_ERROR_WAS_STILL_DRAWING,
        System::Threading::{CreateEventW, SetEvent},
    },
    core::HRESULT,
};

use super::super::{Delivery, window::Readiness};

#[test]
fn busy_hidden_and_late_busy_preserve_one_permit_without_issuing_skipped_work() {
    // An owned auto-reset kernel event exercises real zero-timeout signal consumption;
    // this test creates neither a desktop window nor GPU resources.
    let handle = unsafe { CreateEventW(None, false, false, None) }.unwrap();
    let owned = unsafe { OwnedHandle::from_raw_handle(handle.0) };
    let mut readiness = Readiness::new(owned);
    let conversions = Cell::new(0);
    let accepted = Cell::new(0);
    let submit = |readiness: &mut Readiness, available, result| {
        readiness.admit(available).map(|()| {
            conversions.set(conversions.get() + 1);
            let delivered = readiness.presented(result);
            if delivered == Delivery::Accepted {
                accepted.set(accepted.get() + 1);
            }
            delivered
        })
    };
    assert_eq!(submit(&mut readiness, true, HRESULT(0)), Err(Delivery::Busy));
    // SAFETY: readiness retains this exact event handle for the complete test.
    unsafe { SetEvent(HANDLE(handle.0)) }.unwrap();
    assert_eq!(submit(&mut readiness, false, HRESULT(0)), Err(Delivery::Unavailable));
    assert_eq!(conversions.get(), 0, "busy/hidden outputs cannot admit conversion");
    assert_eq!(
        submit(&mut readiness, true, DXGI_ERROR_WAS_STILL_DRAWING),
        Ok(Delivery::Busy)
    );
    assert_eq!(conversions.get(), 1);
    assert_eq!(accepted.get(), 0);
    assert_eq!(submit(&mut readiness, false, HRESULT(0)), Err(Delivery::Unavailable));
    assert_eq!(conversions.get(), 1);
    // No new signal exists. A late Present Busy retained the already acquired permit.
    assert_eq!(submit(&mut readiness, true, HRESULT(0)), Ok(Delivery::Accepted));
    assert_eq!(conversions.get(), 2);
    assert_eq!(accepted.get(), 1);
    assert_eq!(submit(&mut readiness, true, HRESULT(0)), Err(Delivery::Busy));
    assert_eq!(
        conversions.get(),
        2,
        "accepted presentation consumes the permit exactly once"
    );
    // SAFETY: readiness still owns the event; this signals a new independent frame slot.
    unsafe { SetEvent(HANDLE(handle.0)) }.unwrap();
    assert_eq!(submit(&mut readiness, true, HRESULT(0)), Ok(Delivery::Accepted));
    assert_eq!(conversions.get(), 3);
    assert_eq!(accepted.get(), 2);
}

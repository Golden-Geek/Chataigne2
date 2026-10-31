use std::os::windows::io::{FromRawHandle, OwnedHandle};

use windows::Win32::{
    Foundation::HANDLE,
    Graphics::Dxgi::DXGI_ERROR_WAS_STILL_DRAWING,
    System::Threading::{CreateEventW, SetEvent},
};
use windows::core::HRESULT;

use super::*;
use crate::native::outputs::{Delivery, window::Readiness};

fn event(signaled: bool) -> (Readiness, HANDLE) {
    // SAFETY: no borrowed creation parameters; Readiness owns the resulting auto-reset event.
    let handle = unsafe { CreateEventW(None, false, signaled, None) }.unwrap();
    let owned = unsafe { OwnedHandle::from_raw_handle(handle.0) };
    (Readiness::new(owned), handle)
}

#[test]
fn ready_outputs_deliver_before_window_retry_and_late_present_busy_is_not_retried() {
    let (mut pending, pending_event) = event(false);
    let (mut late_busy, _) = event(true);
    let (mut hidden, _) = event(true);
    let mut schedule = OutputSchedule::new(4);
    let mut outcomes = Vec::new();
    let mut conversion_counts = [0; 4];
    let mut attempts = Vec::new();
    schedule.submit(4, |index, admission| {
        attempts.push((index, admission));
        let result = match index {
            0 => pending.admit(true, admission.timeout_ms()),
            1 => late_busy.admit(true, admission.timeout_ms()),
            2 => {
                // A healthy consumer can deliver and make the previously busy window ready
                // before any positive wait. This also exercises one real signal consumption.
                unsafe { SetEvent(pending_event) }.unwrap();
                Ok(())
            }
            3 => hidden.admit(false, admission.timeout_ms()),
            _ => unreachable!(),
        };
        let delivered = match result {
            Err(Delivery::Busy) if admission == Admission::Poll => return Attempt::DeferredWindow,
            Err(delivered) => delivered,
            Ok(()) => {
                conversion_counts[index] += 1;
                match index {
                    0 => pending.presented(HRESULT(0)),
                    1 => late_busy.presented(DXGI_ERROR_WAS_STILL_DRAWING),
                    2 => Delivery::Accepted,
                    _ => unreachable!(),
                }
            }
        };
        outcomes.push((index, delivered));
        Attempt::Complete
    });
    assert_eq!(conversion_counts, [1, 1, 1, 0]);
    assert_eq!(
        outcomes,
        [
            (1, Delivery::Busy),
            (2, Delivery::Accepted),
            (3, Delivery::Unavailable),
            (0, Delivery::Accepted)
        ]
    );
    assert_eq!(attempts.iter().filter(|(index, _)| *index == 1).count(), 1);
    assert_eq!(attempts.last(), Some(&(0, Admission::Retry { timeout_ms: 1 })));
    assert_eq!(
        pending.admit(true, 0),
        Err(Delivery::Busy),
        "retry consumes its signal once"
    );
    assert_eq!(late_busy.admit(true, 0), Ok(()), "late Present Busy retains its permit");
    assert_eq!(late_busy.presented(HRESULT(0)), Delivery::Accepted);
    assert_eq!(late_busy.admit(true, 0), Err(Delivery::Busy));
    assert_eq!(
        hidden.admit(true, 0),
        Ok(()),
        "hidden admission cannot consume a signal"
    );
}

#[test]
fn many_unsignaled_windows_share_one_positive_timeout_and_record_one_final_outcome_each() {
    let mut windows = (0..128).map(|_| event(false).0).collect::<Vec<_>>();
    let mut schedule = OutputSchedule::new(windows.len());
    let mut final_counts = vec![0; windows.len()];
    let mut positive_timeouts = Vec::new();
    schedule.submit(windows.len(), |index, admission| {
        if admission.timeout_ms() != 0 {
            positive_timeouts.push(admission.timeout_ms());
        }
        assert_eq!(windows[index].admit(true, admission.timeout_ms()), Err(Delivery::Busy));
        if admission == Admission::Poll {
            Attempt::DeferredWindow
        } else {
            final_counts[index] += 1;
            Attempt::Complete
        }
    });
    assert_eq!(positive_timeouts, [1], "wait budget must not multiply by window count");
    assert!(final_counts.iter().all(|count| *count == 1));
}

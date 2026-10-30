//! A new prepared plan may need the same publication while an older frame retains ownership.
use super::{Device, EventCompletion, SourcePool, Texture, TextureDescriptor, TextureId, wait};
use crate::native::AdapterId;
use crate::native::inputs::CopyFrame;
use crate::native::tests::pixels::readback;
use crate::runtime::CompletionToken;

pub(super) fn verify(adapter: AdapterId, pool: &mut SourcePool, id: TextureId, descriptor: TextureDescriptor) {
    let device = Device::new(adapter).unwrap();
    let first_target = Texture::new(&device.device, descriptor).unwrap();
    let next_target = Texture::new(&device.device, descriptor).unwrap();
    let mut completion = EventCompletion::new(&device).unwrap();
    let token = |serial| CompletionToken {
        device: pool.device,
        serial,
    };
    let first_token = token(1);
    let next_token = token(2);
    let first;
    {
        let _submission = completion.begin(&device, first_token);
        first = pool
            .copy(id, &device, &first_target, 1, CopyFrame::default())
            .unwrap()
            .unwrap();
    }
    assert!(first.sequence > 0 && first.timing.is_some());
    assert_eq!(pool.inputs[&id].readers.iter().flatten().count(), 1);
    // Keep the first Reader alive even if the GPU has already completed. The new plan's
    // target has no prior copy, so it cannot skip copying or recursively AcquireSync.
    let next = {
        let _submission = completion.begin(&device, next_token);
        pool.copy(id, &device, &next_target, 2, CopyFrame::default())
    };
    wait("both retained-publication copies complete", || {
        completion.completed(&device, next_token.device).unwrap().serial >= 2
    });
    let retained_after_first = {
        pool.retire(1);
        pool.inputs[&id].readers.iter().flatten().count()
    };
    pool.retire(2); // Always release real completed ownership before checking a failed repro.
    let next = next
        .expect("a new target must share existing renderer ownership, not reacquire the mutex")
        .unwrap();
    assert_eq!(next.sequence, first.sequence);
    assert_eq!(next.timing, first.timing);
    assert_eq!(
        retained_after_first, 1,
        "only the latest actual GPU use can release ownership"
    );
    assert!(pool.inputs[&id].readers.iter().all(Option::is_none));
    let expected = vec![[51, 102, 153, 255]; 64];
    assert_eq!(readback(&device, &first_target), expected);
    assert_eq!(readback(&device, &next_target), expected);
    let shared = pool.inputs[&id].shared.clone();
    assert!(!shared.faulted.load(std::sync::atomic::Ordering::Acquire));
    assert!(!device.removed());
}

pub(super) fn verify_fault_cause(pool: &mut SourcePool, id: TextureId) {
    let shared = pool.inputs[&id].shared.clone();
    let generation = super::snapshot(&shared).generation;
    let invalid_call = windows::Win32::Graphics::Dxgi::DXGI_ERROR_INVALID_CALL;
    pool.fault(id, windows::core::Error::from_hresult(invalid_call));
    wait(
        "logical source fault recovers without fabricated device removal",
        || {
            pool.retire(2);
            let state = super::snapshot(&shared);
            state.generation > generation && state.state == super::SourceState::Streaming
        },
    );
    assert_eq!(super::snapshot(&shared).hresult, Some(invalid_call.0));
}

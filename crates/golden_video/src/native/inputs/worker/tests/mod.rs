//! An actual GPU queue wait, rather than a withheld CPU observation of completed work.
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::sync::mpsc;

use windows::Win32::Foundation::{GENERIC_ALL, HANDLE};
use windows::Win32::Graphics::Direct3D11::{D3D11_FENCE_FLAG_SHARED, ID3D11Device5, ID3D11DeviceContext4, ID3D11Fence};
use windows::core::{Interface, PCWSTR};

use super::*;
use crate::native::{AdapterId, InputLimits, adapters, texture::Texture};
use crate::plan::SpoutFrameCount;

struct QueueGate {
    handle: OwnedHandle,
    fence: ID3D11Fence,
    device: Device,
}

impl QueueGate {
    fn new(adapter: AdapterId) -> Self {
        let device = Device::new(adapter).unwrap();
        let native: ID3D11Device5 = device.device.cast().unwrap();
        let mut fence = None;
        // SAFETY: this test owns the hardware device, shared fence and returned NT handle.
        let (fence, handle) = unsafe {
            native.CreateFence(0, D3D11_FENCE_FLAG_SHARED, &mut fence).unwrap();
            let fence: ID3D11Fence = fence.unwrap();
            let handle = fence.CreateSharedHandle(None, GENERIC_ALL.0, PCWSTR::null()).unwrap();
            (fence, OwnedHandle::from_raw_handle(handle.0))
        };
        Self { handle, fence, device }
    }
}

impl Drop for QueueGate {
    fn drop(&mut self) {
        let context: ID3D11DeviceContext4 = self.device.context.cast().unwrap();
        // SAFETY: always release the test's GPU dependency, including assertion unwind.
        unsafe {
            let _ = context.Signal(&self.fence, 1);
            self.device.context.Flush();
        }
    }
}

#[test]
#[ignore = "requires real D3D11 shared-fence queue waiting on every hardware adapter"]
fn hardware_blocked_gpu_completion_retains_source_owners_until_signal() {
    let (finished, watchdog) = mpsc::channel();
    std::thread::spawn(move || {
        if matches!(
            watchdog.recv_timeout(Duration::from_secs(60)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ) {
            eprintln!("owned source GPU-wait retirement watchdog expired");
            std::process::exit(1);
        }
    });
    for adapter in adapters().unwrap().into_iter().filter(|a| !a.software) {
        let gate = QueueGate::new(adapter.id);
        let handle = gate.handle.as_raw_handle() as usize;
        let (ready, queued) = mpsc::channel();
        let (stop, stopping) = mpsc::channel();
        let (retired, retirement) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            let (mut shared, descriptor) = super::super::tests::fixture();
            let bytes = InputLimits::source_storage_bytes(descriptor).unwrap();
            Arc::get_mut(&mut shared).unwrap().storage_budget = super::super::budget::Budget::new(bytes);
            let binding = SpoutSource {
                sender: format!("GoldenQueuedRetirement-{}", Uuid::new_v4()),
                descriptor,
                metadata: crate::plan::SpoutMetadata::Declared,
                frame_count: SpoutFrameCount::Unversioned,
                poll_interval: Duration::from_millis(5),
            };
            let mut session = Session::new(adapter.id, &binding, shared.clone()).unwrap();
            session.storage = Some(shared.storage_budget.claim(bytes).unwrap());
            session.slots.push(Slot::new(&session.device, descriptor, 0).unwrap());
            shared.update(|status| status.bridge_slots = 1);
            let ownership = Arc::downgrade(&session.slots[0].surface);
            let native: ID3D11Device5 = session.device.device.cast().unwrap();
            let context: ID3D11DeviceContext4 = session.device.context.cast().unwrap();
            let mut fence = None;
            // SAFETY: the controller retains this exact shared handle until worker completion.
            unsafe {
                native.OpenSharedFence(HANDLE(handle as *mut _), &mut fence).unwrap();
                let fence: ID3D11Fence = fence.unwrap();
                context.Wait(&fence, 1).unwrap();
            }
            let token = CompletionToken {
                device: session.generation,
                serial: 1,
            };
            let submission = session.completion.begin(&session.device, token);
            // SAFETY: queued source work targets its own admitted bridge allocation.
            unsafe {
                session
                    .device
                    .context
                    .ClearRenderTargetView(&session.slots[0].texture.rtv, &[1.0, 0.0, 0.0, 1.0]);
            }
            drop(submission);
            session.submitted = 1;
            assert_eq!(
                session
                    .completion
                    .completed(&session.device, session.generation)
                    .unwrap()
                    .serial,
                0
            );
            ready.send((shared.clone(), ownership, bytes)).unwrap();
            let _ = stopping.recv();
            let at = Instant::now();
            drop(session);
            retired.send(at.elapsed()).unwrap();
        });
        let (shared, ownership, bytes) = queued.recv_timeout(Duration::from_secs(6)).unwrap();
        let cancel = Instant::now();
        shared.quiesce();
        shared.shutdown.store(true, Ordering::Release);
        shared.released.store(true, Ordering::Release);
        stop.send(()).unwrap();
        assert!(cancel.elapsed() < Duration::from_millis(50));
        assert!(matches!(
            retirement.recv_timeout(Duration::from_millis(100)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ));
        assert_eq!(ownership.strong_count(), 1, "source owns its unfinished bridge");
        assert!(shared.retiring.load(Ordering::Acquire));
        assert_eq!(shared.status.lock().unwrap().bridge_slots, 1);
        assert!(
            shared.storage_budget.claim(1).is_none(),
            "unfinished native storage remains charged"
        );

        let descriptor = shared.status.lock().unwrap().descriptor;
        let texture = Texture::new(&gate.device.device, descriptor).unwrap();
        let mut healthy = FenceCompletion::new(&gate.device).unwrap();
        let generation = DeviceGeneration::new(Uuid::new_v4(), 0);
        {
            let _submission = healthy.begin(
                &gate.device,
                CompletionToken {
                    device: generation,
                    serial: 1,
                },
            );
            // SAFETY: an independent context on the same GPU proves actual healthy progress.
            unsafe {
                gate.device
                    .context
                    .ClearRenderTargetView(&texture.rtv, &[0.0, 1.0, 0.0, 1.0]);
            }
        }
        let deadline = Instant::now() + Duration::from_secs(2);
        while healthy.completed(&gate.device, generation).unwrap().serial < 1 {
            assert!(
                Instant::now() < deadline,
                "healthy GPU work stalled behind another context"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
        let context: ID3D11DeviceContext4 = gate.device.context.cast().unwrap();
        let release = Instant::now();
        // SAFETY: release only this owned source queue's dependency; never reset a device.
        unsafe {
            context.Signal(&gate.fence, 1).unwrap();
            gate.device.context.Flush();
        }
        let elapsed = retirement.recv_timeout(Duration::from_secs(2)).unwrap();
        worker.join().unwrap();
        assert!(ownership.upgrade().is_none());
        assert_eq!(shared.status.lock().unwrap().bridge_slots, 0);
        assert!(shared.storage_budget.claim(bytes).is_some());
        eprintln!(
            "SOURCE_GPU_WAIT_RETIRE adapter={:?} gpu={:?} total_ms={} release_to_retire_us={} retained_bytes={}",
            adapter.id,
            adapter.description,
            elapsed.as_millis(),
            release.elapsed().as_micros(),
            bytes
        );
    }
    finished.send(()).unwrap();
}

use windows::Win32::Foundation::HANDLE;
use windows::Win32::Graphics::Direct3D11::ID3D11Texture2D;
use windows::Win32::Graphics::Dxgi::IDXGIKeyedMutex;
use windows::core::Interface;

use super::Captures;
use super::{
    AdapterId, ConsumerTick, DiagnosticDelivery, Recorder, collect, consumer, diagnostics, source_with_wait, stamp,
    wait,
};
use crate::native::device::Device;
use crate::native::timing::SpoutImageVersion;

struct GpuOwner {
    mutex: IDXGIKeyedMutex,
    _device: Device,
}

impl GpuOwner {
    fn new(adapter: AdapterId, image: SpoutImageVersion) -> Self {
        let device = Device::new(adapter).unwrap();
        // SAFETY: this live SDK legacy handle belongs to the sender owned by this test. The
        // SDK's 32-bit sender-info contract uses LongToHandle on x64. No offline handle is opened.
        let mut texture = None;
        unsafe {
            device
                .device
                .OpenSharedResource::<ID3D11Texture2D>(
                    HANDLE(image.shared_handle as i32 as isize as *mut _),
                    &mut texture,
                )
                .unwrap();
        }
        let mutex: IDXGIKeyedMutex = texture
            .unwrap()
            .cast()
            .expect("owned Spout output must expose GPU keyed synchronization");
        // SAFETY: a live owned interface and zero timeout; inspect S_OK exactly because a
        // positive WAIT_TIMEOUT HRESULT must not be erased by Result<()> conversion.
        let acquired = unsafe { (Interface::vtable(&mutex).AcquireSync)(Interface::as_raw(&mutex), 0, 0) };
        assert_eq!(acquired, windows::Win32::Foundation::S_OK);
        Self { mutex, _device: device }
    }
}

impl Drop for GpuOwner {
    fn drop(&mut self) {
        // SAFETY: this device acquired ownership and queued no commands on the shared image.
        unsafe { self.mutex.ReleaseSync(0) }.unwrap();
    }
}

pub(super) fn verify(adapter: AdapterId, timeout_ms: u32) -> serde_json::Value {
    let name = format!("GoldenKeyedBusy-{}", uuid::Uuid::new_v4());
    let clock = Recorder::new(diagnostics());
    let mut source = source_with_wait(adapter, &name, timeout_ms);
    assert_eq!(source.snapshot().send_access_timeout_ms, timeout_ms);
    let first = source.submit(stamp(&clock, 1)).unwrap().unwrap();
    wait("initial keyed image completion", || {
        source.poll().unwrap().serial >= first.token.serial
    });
    let mut consumer = consumer(adapter, &name);
    wait("initial keyed image capture", || {
        matches!(consumer.tick().unwrap(), ConsumerTick::Logged { .. })
    });
    wait("initial keyed capture completed", || {
        consumer.poll().unwrap();
        consumer.snapshot().last_completed.serial == consumer.snapshot().last_submitted.serial
    });
    let busy;
    {
        let _owner = GpuOwner::new(adapter, first.spout_image.unwrap());
        let batches = consumer.snapshot().submitted_batches;
        assert!(
            matches!(consumer.tick().unwrap(), ConsumerTick::Unchanged),
            "unchanged CPU hint must not acquire another device's GPU image"
        );
        assert_eq!(consumer.snapshot().submitted_batches, batches);
        busy = source.submit(stamp(&clock, 2)).unwrap().unwrap();
        assert_eq!(busy.outcome, DiagnosticDelivery::Busy);
        assert_eq!(busy.spout_image, None, "busy access must not publish a copy/version");
    }
    let mut next = None;
    let mut serial = 3;
    wait("send resumes after GPU ownership release", || {
        let submission = source.submit(stamp(&clock, serial)).unwrap();
        serial += 1;
        next = submission.filter(|submission| submission.outcome == DiagnosticDelivery::Accepted);
        next.is_some()
    });
    let next = next.unwrap();
    assert_eq!(next.spout_image.unwrap().frame, first.spout_image.unwrap().frame + 1);
    wait("resumed keyed send completion", || {
        source.poll().unwrap().serial >= next.token.serial
    });
    wait("resumed keyed capture", || {
        matches!(consumer.tick().unwrap(), ConsumerTick::Logged { .. })
    });
    source.finish().unwrap();
    let frames = consumer.finish().unwrap();
    assert_eq!(frames.len(), 2);
    let mut report = collect(Captures::direct(frames), &[first, busy, next]);
    assert!(report["mismatches"].as_array().unwrap().is_empty());
    report["busy_outcome"] = serde_json::json!(busy.outcome);
    report["send_access_timeout_ms"] = serde_json::json!(timeout_ms);
    report["counter_after_resume"] = serde_json::json!(next.spout_image.unwrap().frame);
    report["unchanged_hint_while_gpu_owned"] = serde_json::json!(true);
    report
}

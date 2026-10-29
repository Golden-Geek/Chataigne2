use std::num::NonZeroUsize;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

use uuid::Uuid;
use windows::Win32::Foundation::{HANDLE, WAIT_OBJECT_0};
use windows::Win32::System::Threading::{MUTEX_ALL_ACCESS, OpenMutexW, ReleaseMutex, WaitForSingleObject};
use windows::core::PCWSTR;

use super::super::shared::Shared;
use super::super::{SourcePool, SourceSnapshot, SourceState};
use crate::native::completion::EventCompletion;
use crate::native::device::Device;
use crate::native::outputs::Delivery;
use crate::native::outputs::spout::SpoutOutput;
use crate::native::texture::Texture;
use crate::native::timing::{DiagnosticContext, DiagnosticEvent, DiagnosticsConfig, Recorder};
use crate::native::{AdapterId, D3d11Control, InputLimits, adapters};
use crate::plan::{ExternalTexture, PlanLimits, RenderPlan, SpoutFrameCount, SpoutSource};
use crate::runtime::{CompletionToken, DeviceGeneration};
use crate::{AlphaMode, ColorEncoding, Extent2D, PixelFormat, TextureDescriptor, TextureId};

const WAIT: Duration = Duration::from_secs(6);

fn descriptor(side: u32) -> TextureDescriptor {
    TextureDescriptor {
        extent: Extent2D::new(side, side).unwrap(),
        format: PixelFormat::Rgba8Unorm,
        color_encoding: ColorEncoding::Linear,
        alpha_mode: AlphaMode::Premultiplied,
    }
}

fn wait(label: &str, mut ready: impl FnMut() -> bool) {
    let deadline = Instant::now() + WAIT;
    while !ready() {
        assert!(Instant::now() < deadline, "timed out: {label}");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn snapshot(shared: &Shared) -> SourceSnapshot {
    *shared.status.lock().unwrap()
}

/// Sends exactly once. Its positive counter remains frozen while both receiver policies run.
/// Every attempted send remains fenced through real completion before SDK storage is dropped.
struct Sender {
    output: Option<SpoutOutput>,
    texture: Texture,
    device: Device,
    completion: EventCompletion,
    token: CompletionToken,
}

impl Sender {
    fn new(adapter: AdapterId, name: String, descriptor: TextureDescriptor) -> Self {
        let device = Device::new(adapter).unwrap();
        let texture = Texture::new(&device.device, descriptor).unwrap();
        let output = SpoutOutput::new(&device.device, name, descriptor).unwrap();
        let completion = EventCompletion::new(&device).unwrap();
        let token = CompletionToken {
            device: DeviceGeneration::new(Uuid::new_v4(), 0),
            serial: 1,
        };
        let mut sender = Self {
            output: Some(output),
            texture,
            device,
            completion,
            token,
        };
        let delivered;
        {
            let _submission = sender.completion.begin(&sender.device, token);
            // SAFETY: this test thread owns the source texture and its immediate context.
            unsafe {
                sender
                    .device
                    .context
                    .ClearRenderTargetView(&sender.texture.rtv, &[0.2, 0.4, 0.6, 1.0]);
            }
            delivered = sender
                .output
                .as_mut()
                .unwrap()
                .deliver(&sender.texture, None, &mut None);
        }
        wait("one-shot sender completion", || {
            sender
                .completion
                .completed(&sender.device, token.device)
                .unwrap()
                .serial
                >= token.serial
        });
        assert_eq!(delivered, Delivery::Accepted);
        sender
    }
}

impl Drop for Sender {
    fn drop(&mut self) {
        // A test deadline cannot authorize releasing an outstanding native image copy.
        while !self.device.removed()
            && !self
                .completion
                .completed(&self.device, self.token.device)
                .is_ok_and(|completed| completed.serial >= self.token.serial)
        {
            std::thread::sleep(Duration::from_millis(1));
        }
        self.output.take();
    }
}

/// The standard owned SendTexture sender uses legacy shared storage and this named mutex.
/// Acquire it on the test thread, distinct from the receiver workers, to force real SDK Busy.
struct AccessLock(OwnedHandle);

impl AccessLock {
    fn new(sender: &str) -> Self {
        let name: Vec<u16> = format!("{sender}_SpoutAccessMutex\0").encode_utf16().collect();
        // SAFETY: a terminated owned sender name; this test never opens a foreign endpoint lock.
        let handle = unsafe { OpenMutexW(MUTEX_ALL_ACCESS, false, PCWSTR(name.as_ptr())) }.unwrap();
        let lock = Self(unsafe { OwnedHandle::from_raw_handle(handle.0) });
        assert_eq!(unsafe { WaitForSingleObject(handle, 2000) }, WAIT_OBJECT_0);
        lock
    }
}

impl Drop for AccessLock {
    fn drop(&mut self) {
        // SAFETY: acquired on this same test thread; release before workers are retired.
        unsafe { ReleaseMutex(HANDLE(self.0.as_raw_handle())) }.unwrap();
    }
}

#[test]
#[ignore = "requires hardware D3D11 and an owned Spout sender/access mutex"]
fn hardware_versioned_preflight_and_busy_preserve_source_publications() {
    let (_done, stop) = mpsc::channel::<()>();
    std::thread::spawn(move || {
        if matches!(
            stop.recv_timeout(Duration::from_secs(60)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ) {
            eprintln!("owned source freshness test exceeded 60 seconds");
            std::process::exit(1);
        }
    });
    let adapter = adapters()
        .unwrap()
        .into_iter()
        .find(|adapter| !adapter.software)
        .unwrap()
        .id;
    let name = format!("GoldenFreshness-{}", Uuid::new_v4());
    let sender = Sender::new(adapter, name.clone(), descriptor(8));
    let versioned = TextureId::new(1).unwrap();
    let foreign = TextureId::new(2).unwrap();
    let plan = RenderPlan {
        external_textures: [
            (versioned, SpoutFrameCount::VersionedV1),
            (foreign, SpoutFrameCount::Unversioned),
        ]
        .into_iter()
        .map(|(id, frame_count)| ExternalTexture {
            id,
            descriptor: descriptor(8),
            source: Some(
                SpoutSource {
                    sender: name.clone(),
                    descriptor: descriptor(8),
                    poll_interval: Duration::from_millis(5),
                    frame_count,
                }
                .into(),
            ),
        })
        .collect(),
        ..Default::default()
    }
    .compile_with_source_demands(
        &PlanLimits {
            max_passes: 0,
            max_external_textures: 2,
            max_outputs: 0,
            max_dependencies: 0,
            max_texture_dimension: 8,
            max_texture_bytes: 1024,
        },
        &[versioned, foreign],
    )
    .unwrap();
    let control = D3d11Control::default();
    let recorder = Recorder::new(DiagnosticsConfig {
        capacity: NonZeroUsize::new(4096).unwrap(),
    });
    *control.diagnostics.lock().unwrap() = Some(recorder.clone());
    let mut pool = SourcePool::new(
        adapter,
        InputLimits {
            max_sources: 2,
            max_storage_bytes: 2560,
        },
        &control,
        None,
        DeviceGeneration::new(Uuid::new_v4(), 0),
    );
    pool.ensure(&plan).unwrap();
    let counted = Arc::clone(&pool.inputs[&versioned].shared);
    let unknown = Arc::clone(&pool.inputs[&foreign].shared);
    let counted_demand = counted.acquire_demand();
    let unknown_demand = unknown.acquire_demand();
    wait("first copy and unchanged preflight", || {
        snapshot(&counted).unchanged >= 8
    });
    wait("foreign frozen-positive counter still copies", || {
        snapshot(&unknown).publications >= 3
    });
    // The unversioned receiver deliberately keeps taking the same sender's access mutex.
    // A contended peek may proceed to a real receive attempt, so measure the unchanged
    // fast path only after that competing receiver has fenced its last copy and gone idle.
    drop(unknown_demand);
    wait("foreign receiver idle before uncontended preflight", || {
        snapshot(&unknown).state == SourceState::Idle
    });
    let before = snapshot(&counted);
    assert_eq!(before.last_copied_frame, Some(1));
    assert_eq!(before.receive_copies, 1);
    assert_eq!(snapshot(&unknown).last_copied_frame, None);
    let mut rows = Vec::new();
    recorder.drain(&mut rows, NonZeroUsize::new(4096).unwrap());
    wait("additional unchanged polls", || {
        snapshot(&counted).unchanged >= before.unchanged + 8
    });
    let after = snapshot(&counted);
    assert_eq!(
        (after.sdk_calls, after.receive_copies, after.publications),
        (before.sdk_calls, before.receive_copies, before.publications)
    );
    rows.clear();
    recorder.drain(&mut rows, NonZeroUsize::new(4096).unwrap());
    assert!(
        !rows.iter().any(|row| matches!(row, DiagnosticEvent::SourceCpu(trace)
        if matches!(trace.context, DiagnosticContext::Source { texture, .. } if texture == versioned))),
        "unchanged preflight must not submit a new source query/Flush batch"
    );
    let unknown_demand = unknown.acquire_demand();
    wait("foreign receiver resumed", || {
        snapshot(&unknown).state == SourceState::Streaming
    });
    let demand = (counted_demand, unknown_demand);

    // No sender work occurs during this pulse. It may be shorter than a worker poll, but
    // clearing demand synchronously withdraws the mailbox and must not strand counter 1.
    let before_pulse = snapshot(&counted);
    drop(demand);
    assert!(counted.latest.lock().unwrap().is_none());
    assert!(unknown.latest.lock().unwrap().is_none());
    let _demand = pool.demands(&plan);
    wait("static sender republishes after a short demand pulse", || {
        snapshot(&counted).publications > before_pulse.publications
    });
    assert_eq!(snapshot(&counted).last_copied_frame, Some(1));

    // Deterministically reproduce the same postcondition as a publication suppressed by
    // demand dropping during receive: completed copied history, active demand, no mailbox.
    let before_suppressed = snapshot(&counted);
    counted.clear();
    wait("cached static copy without a publication is re-armed", || {
        snapshot(&counted).publications > before_suppressed.publications
    });
    assert_eq!(snapshot(&counted).last_copied_frame, Some(1));
    assert!(snapshot(&counted).sequence > before_suppressed.sequence);

    {
        let _access = AccessLock::new(&name);
        // Both workers have already copied. Waiting for a first Busy also drains any preceding
        // successful publication before testing the retained-new-frame flag on a second timeout.
        wait("first actual SDK Busy", || {
            snapshot(&counted).receive_busy >= 1 && snapshot(&unknown).receive_busy >= 1
        });
        let stable = [snapshot(&counted), snapshot(&unknown)];
        let sequence = [
            counted.latest.lock().unwrap().as_ref().unwrap().sequence,
            unknown.latest.lock().unwrap().as_ref().unwrap().sequence,
        ];
        wait("repeated actual SDK Busy", || {
            snapshot(&counted).receive_busy > stable[0].receive_busy
                && snapshot(&unknown).receive_busy > stable[1].receive_busy
        });
        for (index, shared) in [&counted, &unknown].into_iter().enumerate() {
            let current = snapshot(shared);
            assert_eq!(current.state, SourceState::Streaming);
            assert_eq!(
                (current.publications, current.receive_copies, current.sequence),
                (
                    stable[index].publications,
                    stable[index].receive_copies,
                    stable[index].sequence
                )
            );
            assert_eq!(
                shared.latest.lock().unwrap().as_ref().unwrap().sequence,
                sequence[index]
            );
        }
    }
    let foreign_copies = snapshot(&unknown).receive_copies;
    wait("copy resumes after access release", || {
        snapshot(&unknown).receive_copies > foreign_copies
    });

    // Same exact name, smaller replacement, and the same positive initial counter: stale
    // equality history must not suppress its first copy or inherit the old bridge pool.
    let previous = [snapshot(&counted), snapshot(&unknown)];
    drop(sender);
    wait("both receivers close the disconnected counter lifetime", || {
        [&counted, &unknown].into_iter().all(|shared| {
            snapshot(shared).state == SourceState::WaitingForSender && shared.latest.lock().unwrap().is_none()
        })
    });
    let _replacement = Sender::new(adapter, name, descriptor(4));
    wait("replacement layout and counter lifetime", || {
        pool.retire(0);
        [&counted, &unknown].into_iter().enumerate().all(|(index, shared)| {
            let current = snapshot(shared);
            current.state == SourceState::Streaming
                && current.observed_descriptor == Some(descriptor(4))
                && current.pool_generation > previous[index].pool_generation
                && current.receive_copies > previous[index].receive_copies
        })
    });
    assert_eq!(snapshot(&counted).last_copied_frame, Some(1));
    assert_eq!(recorder.snapshot().dropped_full, 0);
}

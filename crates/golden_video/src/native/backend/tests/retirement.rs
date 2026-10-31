//! Completion-unavailable retirement with real native resources and an independent healthy path.
use std::num::NonZeroUsize;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use super::startup::{config, descriptor, scene};

mod removal;
use crate::native::tests::completion_gate::CompletionGate;
use crate::native::{D3d11Config, D3d11Control, InputLimits, OutputBinding, SpoutProbe, adapters};
use crate::runtime::{LatePolicy, OutcomeKind, PlanActivation, RuntimeState, VideoRuntime};
use crate::{AlphaMode, ColorEncoding, OutputId, PixelFormat, TextureDescriptor};
use golden_io::PayloadPool;

const WAIT: Duration = Duration::from_secs(5);

fn wait(label: &str, mut ready: impl FnMut() -> bool) {
    let deadline = Instant::now() + WAIT;
    while !ready() {
        assert!(Instant::now() < deadline, "native retirement deadline: {label}");
        std::thread::sleep(Duration::from_millis(5));
    }
}

struct Owner {
    runtime: Option<VideoRuntime>,
    gate: CompletionGate,
    control: D3d11Control,
    name: String,
    plan: crate::plan::RenderPlan,
}
impl Owner {
    fn new(adapter: crate::native::AdapterId, held: bool) -> Self {
        Self::configured(adapter, held, config(), scene(), InputLimits::default())
    }
    fn configured(
        adapter: crate::native::AdapterId,
        held: bool,
        config: crate::runtime::RuntimeConfig,
        plan: crate::plan::RenderPlan,
        inputs: InputLimits,
    ) -> Self {
        let control = D3d11Control::default();
        let gate = control.completion_gate.clone();
        if held {
            gate.hold();
        }
        let name = format!("GoldenRetirement-{}", uuid::Uuid::new_v4());
        let runtime = VideoRuntime::spawn_d3d11_with_control(
            config,
            D3d11Config {
                adapter,
                diagnostics: None,
                inputs,
                outputs: vec![OutputBinding::Spout {
                    id: OutputId::new(1).unwrap(),
                    name: name.clone(),
                    descriptor: TextureDescriptor {
                        format: PixelFormat::Bgra8Unorm,
                        color_encoding: ColorEncoding::Srgb,
                        alpha_mode: AlphaMode::Straight,
                        ..descriptor()
                    },
                }],
            },
            control.clone(),
        )
        .unwrap();
        Self {
            runtime: Some(runtime),
            gate,
            control,
            name,
            plan,
        }
    }
    fn runtime(&self) -> &VideoRuntime {
        self.runtime.as_ref().unwrap()
    }
    fn submit(&mut self) {
        let runtime = self.runtime.as_mut().unwrap();
        let request = runtime
            .submit_plan(
                runtime.project_epoch(),
                self.plan.clone(),
                PlanActivation::NextFrame(runtime.read_time().unwrap().epoch()),
                LatePolicy::Activate,
            )
            .unwrap();
        let mut installed = false;
        wait("native installed/submitted plan", || {
            let mut outcomes = Vec::new();
            runtime.drain_outcomes(&mut outcomes, NonZeroUsize::new(4).unwrap());
            for outcome in outcomes {
                assert_eq!(outcome.request, request);
                assert!(matches!(outcome.result, OutcomeKind::Installed { .. }), "{outcome:?}");
                installed = true;
            }
            installed && runtime.snapshot().frames_submitted >= 2
        });
    }
}
impl Drop for Owner {
    fn drop(&mut self) {
        // Release the observation seam first, including during a failed assertion's unwind.
        self.gate.release();
        if let Some(runtime) = self.runtime.take() {
            let retirement = runtime.retirement();
            runtime.close();
            drop(runtime);
            while !retirement.wait(Duration::from_millis(20)) {}
        }
    }
}

#[test]
#[ignore = "explicit hardware D3D11 completion-delay/retirement qualification; does not remove a GPU"]
fn hardware_unobserved_completion_retains_owners_and_project_ack_while_healthy_output_continues() {
    let (done, finished) = mpsc::channel();
    std::thread::spawn(move || {
        if matches!(
            finished.recv_timeout(Duration::from_secs(60)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ) {
            eprintln!("native completion-delay qualification/retirement watchdog expired");
            std::process::exit(1);
        }
    });
    let hardware = adapters()
        .unwrap()
        .into_iter()
        .filter(|adapter| !adapter.software)
        .collect::<Vec<_>>();
    assert!(!hardware.is_empty());
    for adapter in hardware {
        let mut healthy = Owner::new(adapter.id, false);
        healthy.submit();
        let mut delayed = Owner::new(adapter.id, true);
        delayed.submit();
        wait("bounded native in-flight queue reaches its admitted limit", || {
            delayed.runtime().snapshot().resources.unwrap().in_flight == 2
        });
        let original = delayed.runtime().snapshot().resources.unwrap();
        assert_eq!(original.in_flight, 2);
        assert_eq!(original.last_completed.serial, 0);
        let last_use = original.last_submitted;
        wait("real GPU query observed behind closed delivery gate", || {
            delayed
                .gate
                .snapshot()
                .observed
                .is_some_and(|token| token.device == last_use.device && token.serial >= last_use.serial)
        });
        let (request, project) = delayed.runtime.as_mut().unwrap().replace_project().unwrap();
        wait("old resources retained for unacknowledged project replacement", || {
            let resources = delayed.runtime().snapshot().resources.unwrap();
            resources.active.is_none() && resources.retiring_generations == 1 && resources.in_flight == 2
        });
        let mut outcomes = Vec::new();
        delayed
            .runtime()
            .drain_outcomes(&mut outcomes, NonZeroUsize::new(4).unwrap());
        assert!(outcomes.is_empty(), "project acknowledgement must wait for completion");
        let retirement = delayed.runtime().retirement();
        let close = Instant::now();
        delayed.runtime().close();
        let close_elapsed = close.elapsed();
        assert!(
            close_elapsed < Duration::from_millis(50),
            "close blocked: {close_elapsed:?}"
        );
        wait("worker stopping without fabricated retirement", || {
            delayed.runtime().snapshot().state == RuntimeState::Stopping
        });
        assert!(
            !retirement.wait(Duration::from_millis(100)),
            "timeout must not retire native owners"
        );
        let retained = delayed.runtime().snapshot().resources.unwrap();
        assert!(retained.bytes_used > 0 && retained.bytes_used <= 65536);
        assert_eq!(retained.last_completed.serial, 0);
        assert!(!retained.device_lost);
        assert!(delayed.control.snapshot().initialized);
        assert_eq!(delayed.control.snapshot().device, Some(last_use.device));
        delayed
            .runtime()
            .drain_outcomes(&mut outcomes, NonZeroUsize::new(4).unwrap());
        assert!(outcomes.is_empty());
        let healthy_before = healthy.runtime().snapshot().frames_submitted;
        let accepted_before = healthy.control.snapshot().accepted_outputs;
        wait("independent healthy output continues", || {
            healthy.runtime().snapshot().frames_submitted >= healthy_before + 3
                && healthy.control.snapshot().accepted_outputs >= accepted_before + 3
        });
        let budget = PayloadPool::new("native-retirement-pixels", 3, 65536).unwrap();
        let mut probe = SpoutProbe::new(&healthy.name, adapter.id, budget.clone()).unwrap();
        let mut pixels = None;
        wait("independent healthy pixels", || {
            pixels = probe.capture_pixels().unwrap();
            pixels.is_some()
        });
        let pixels = pixels.unwrap();
        assert_eq!(pixels.extent(), descriptor().extent);
        assert!(pixels.pixels().iter().all(|pixel| *pixel == [0, 255, 0, 255]));
        drop(pixels);
        drop(probe);
        assert!(budget.wait_for_idle(WAIT));
        let release = Instant::now();
        delayed.gate.release();
        assert!(retirement.wait(WAIT));
        let release_elapsed = release.elapsed();
        let stopped = delayed.runtime().snapshot();
        assert_eq!(stopped.state, RuntimeState::Stopped);
        assert_eq!(stopped.resources.unwrap().bytes_used, 0);
        assert!(!delayed.control.snapshot().initialized);
        delayed
            .runtime()
            .drain_outcomes(&mut outcomes, NonZeroUsize::new(4).unwrap());
        assert_eq!(outcomes.len(), 1);
        assert_eq!(outcomes[0].request, request);
        assert!(matches!(outcomes[0].result, OutcomeKind::ProjectChanged(current) if current == project));
        assert!(close.elapsed() < WAIT, "complete test shutdown exceeded five seconds");
        eprintln!(
            "native held-observation retirement adapter={:?} description={} real_query={:?} retained_bytes={} close_us={} release_to_retire_ms={} total_close_ms={} healthy_frames={}",
            adapter.id,
            adapter.description,
            delayed.gate.snapshot(),
            retained.bytes_used,
            close_elapsed.as_micros(),
            release_elapsed.as_millis(),
            close.elapsed().as_millis(),
            healthy.runtime().snapshot().frames_submitted - healthy_before
        );
        drop(delayed);
        drop(healthy);
    }
    done.send(()).unwrap();
}

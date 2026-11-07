use std::time::{Duration, Instant};

use golden_io::PayloadPool;
use golden_time::RationalRate;

use crate::OutputId;
use crate::native::{D3d11Config, D3d11Control, InputLimits, OutputBinding, SpoutProbe, adapters};
use crate::plan::{RegionPlacement, RenderPlan, UnitRect};
use crate::runtime::{LatePolicy, PlanActivation, VideoRuntime};

struct Running(Option<VideoRuntime>);
impl Drop for Running {
    fn drop(&mut self) {
        if let Some(runtime) = self.0.take() {
            let retired = runtime.retirement();
            runtime.close();
            drop(runtime);
            while !retired.wait(Duration::from_millis(20)) {}
        }
    }
}

fn wait(mut ready: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !ready() {
        assert!(
            Instant::now() < deadline,
            "presentation did not advance independently of the minute-long media deadline"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
#[ignore = "explicit hardware D3D11 cached-presentation/slow-media pixels and independent-output acceptance on both adapters"]
fn hardware_live_presentation_wakes_between_media_frames_without_resubmitting_an_independent_output() {
    let (done, stop) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        if matches!(
            stop.recv_timeout(Duration::from_secs(45)),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout)
        ) {
            eprintln!("owned redraw test watchdog expired");
            std::process::exit(1);
        }
    });
    let hardware: Vec<_> = adapters()
        .unwrap()
        .into_iter()
        .filter(|adapter| !adapter.software)
        .collect();
    assert!(!hardware.is_empty());
    for adapter in hardware {
        let placement = RegionPlacement::new(UnitRect::FULL, UnitRect::FULL).unwrap();
        let compiled = super::presentation::scene(&placement);
        let descriptor = compiled.passes()[0].descriptor;
        let mut config = super::startup::config();
        config.frame_rate = RationalRate::new(1, 60).unwrap();
        config.plan_limits.max_passes = 4;
        config.plan_limits.max_outputs = 2;
        config.plan_limits.max_dependencies = 8;
        let names = [
            format!("GoldenRedraw-{}-view", uuid::Uuid::new_v4()),
            format!("GoldenRedraw-{}-peer", uuid::Uuid::new_v4()),
        ];
        let control = D3d11Control::default();
        let mut running = Running(Some(
            VideoRuntime::spawn_d3d11_with_control(
                config,
                D3d11Config {
                    diagnostics: None,
                    adapter: adapter.id,
                    inputs: InputLimits::default(),
                    outputs: names
                        .iter()
                        .enumerate()
                        .map(|(i, name)| OutputBinding::Spout {
                            id: OutputId::new(i as u64 + 1).unwrap(),
                            name: name.clone(),
                            descriptor,
                        })
                        .collect(),
                },
                control.clone(),
            )
            .unwrap(),
        ));
        let runtime = running.0.as_mut().unwrap();
        runtime
            .submit_plan(
                runtime.project_epoch(),
                RenderPlan {
                    passes: compiled.passes().to_vec(),
                    outputs: compiled.outputs().to_vec(),
                    ..Default::default()
                },
                PlanActivation::NextFrame(runtime.read_time().unwrap().epoch()),
                LatePolicy::Activate,
            )
            .unwrap();
        wait(|| {
            runtime.snapshot().frames_submitted == 1
                && control
                    .output_snapshots()
                    .iter()
                    .all(|output| output.accepted_frames == 1)
                && control.output_snapshots().len() == 2
        });
        let first = runtime.snapshot();
        let allocations = control.snapshot().created_textures;
        let initial = control.output_snapshots();
        for step in 0..24 {
            let columns = if step % 2 == 0 { 2 } else { 3 };
            let before = control.output_snapshots()[0].accepted_frames;
            assert!(placement.update(
                UnitRect {
                    width: columns as f32 / 4.0,
                    ..UnitRect::FULL
                },
                UnitRect::FULL
            ));
            wait(|| control.output_snapshots()[0].accepted_frames > before);
            // Readiness probes capture one image. A fresh bounded probe is required for
            // each expected image; reusing it would keep comparing the first snapshot.
            let mut view = SpoutProbe::new(
                &names[0],
                adapter.id,
                PayloadPool::new("redraw-view", 3, 65536).unwrap(),
            )
            .unwrap();
            let mut peer = SpoutProbe::new(
                &names[1],
                adapter.id,
                PayloadPool::new("redraw-peer", 3, 65536).unwrap(),
            )
            .unwrap();
            wait(|| {
                view.capture_pixels().unwrap().is_some_and(|image| {
                    image
                        .pixels()
                        .iter()
                        .enumerate()
                        .all(|(index, pixel)| *pixel == if index % 4 < columns { [255, 0, 0, 255] } else { [0; 4] })
                })
            });
            wait(|| {
                peer.capture_pixels()
                    .unwrap()
                    .is_some_and(|image| image.pixels() == [[0, 255, 0, 255]; 16])
            });
            assert_eq!(
                runtime.snapshot().frames_submitted,
                1,
                "redraw does not advance the media frame counter"
            );
            assert_eq!(
                control.output_snapshots()[1].accepted_frames,
                initial[1].accepted_frames,
                "independent output is not resubmitted"
            );
            assert_eq!(
                control.snapshot().created_textures,
                allocations,
                "redraw reuses admitted textures"
            );
            assert_eq!(
                runtime.snapshot().resources.unwrap().active,
                first.resources.unwrap().active
            );
            assert_eq!(
                runtime.snapshot().last_frame.unwrap().completion,
                first.last_frame.unwrap().completion
            );
        }
        let deliveries = control.output_snapshots()[0].accepted_frames;
        std::thread::sleep(Duration::from_millis(25));
        assert_eq!(
            control.output_snapshots()[0].accepted_frames,
            deliveries,
            "unchanged idle geometry does not keep redrawing"
        );
        eprintln!(
            "presentation adapter {:?} {}:24 cached redraws/pixels, unchanged one media frame/peer, stable allocations",
            adapter.id, adapter.description
        );
        drop(running);
        assert!(control.output_snapshots().is_empty());
    }
    done.send(()).unwrap();
}

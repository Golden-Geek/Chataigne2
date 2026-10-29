use std::num::NonZeroUsize;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use uuid::Uuid;

use super::super::*;
use crate::native::{adapters, completion::EventCompletion, device::Device, texture::Texture};
use crate::runtime::{CompletionToken, DeviceGeneration};
use crate::{AlphaMode, ColorEncoding, Extent2D, PixelFormat, TextureDescriptor};

struct Drain<'a> {
    device: &'a Device,
    completion: &'a mut EventCompletion,
    timing: &'a mut GpuTiming,
    generation: DeviceGeneration,
    submitted: u64,
}

impl Drop for Drain<'_> {
    fn drop(&mut self) {
        while !self.device.removed() {
            if let Ok(done) = self.completion.completed(self.device, self.generation) {
                self.timing.poll(&self.device.context, done);
                if done.serial >= self.submitted {
                    break;
                }
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}

#[test]
#[ignore = "requires hardware D3D11"]
fn hardware_diagnostics_queries_keep_two_slots_and_collect_exact_completed_context_tokens() {
    let (done, stop) = mpsc::channel();
    std::thread::spawn(move || {
        if matches!(
            stop.recv_timeout(Duration::from_secs(60)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ) {
            eprintln!("owned diagnostic query test exceeded 60 seconds");
            std::process::exit(1);
        }
    });
    let adapter = adapters()
        .unwrap()
        .into_iter()
        .find(|adapter| !adapter.software)
        .unwrap()
        .id;
    let device = Device::new(adapter).unwrap();
    let texture = Texture::new(
        &device.device,
        TextureDescriptor {
            extent: Extent2D::new(8, 8).unwrap(),
            format: PixelFormat::Rgba8Unorm,
            color_encoding: ColorEncoding::Linear,
            alpha_mode: AlphaMode::Premultiplied,
        },
    )
    .unwrap();
    let recorder = Recorder::new(DiagnosticsConfig {
        capacity: NonZeroUsize::new(16).unwrap(),
    });
    let mut timing = GpuTiming::new(
        &device.device,
        recorder.clone(),
        DiagnosticContext::FixtureProducer { id: Uuid::nil() },
    );
    let mut completion = EventCompletion::new(&device).unwrap();
    let generation = DeviceGeneration::new(Uuid::new_v4(), 1);
    let mut drain = Drain {
        device: &device,
        completion: &mut completion,
        timing: &mut timing,
        generation,
        submitted: 0,
    };
    let cpu = CpuScope::new(&recorder);
    for serial in 1..=2 {
        drain.submitted = serial;
        let token = CompletionToken {
            device: generation,
            serial,
        };
        let _completion = drain.completion.begin(&device, token);
        let _timing = drain.timing.begin(&device.context, token);
        // SAFETY: this test owns the context and target; no pixel readback is performed.
        unsafe {
            device
                .context
                .ClearRenderTargetView(&texture.rtv, &[0.0, 1.0, 0.0, 1.0]);
        }
    }
    drop(drain.timing.begin(
        &device.context,
        CompletionToken {
            device: generation,
            serial: 3,
        },
    ));
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let completed = drain.completion.completed(&device, generation).unwrap();
        drain.timing.poll(&device.context, completed);
        if completed.serial == 2 && recorder.snapshot().written >= 3 {
            break;
        }
        assert!(Instant::now() < deadline, "finite diagnostic query drain");
        std::thread::sleep(Duration::from_millis(1));
    }
    let measured = cpu.finish();
    assert!(measured.wall.is_some_and(|wall| wall.end >= wall.begin));
    assert!(measured.thread_cpu.is_some());
    let mut rows = Vec::new();
    recorder.drain(&mut rows, NonZeroUsize::new(16).unwrap());
    let gpu = rows
        .iter()
        .map(|row| match row {
            DiagnosticEvent::Gpu(row) => *row,
            _ => unreachable!(),
        })
        .collect::<Vec<_>>();
    assert_eq!(gpu.len(), 3);
    assert!(
        gpu.iter()
            .any(|row| row.token.serial == 3 && row.status == GpuSampleStatus::PoolBusy)
    );
    for serial in 1..=2 {
        let row = gpu.iter().find(|row| row.token.serial == serial).unwrap();
        assert_eq!(row.token.device, generation);
        assert_eq!(row.context, DiagnosticContext::FixtureProducer { id: Uuid::nil() });
        assert_eq!(row.status, GpuSampleStatus::Valid);
        assert!(row.frequency.is_some_and(|frequency| frequency > 0));
        assert!(row.begin.zip(row.end).is_some_and(|(begin, end)| end >= begin));
        assert_eq!((row.renderer_inputs_end, row.renderer_render_end), (None, None));
    }
    assert_eq!(recorder.snapshot().dropped_full, 0);
    drop(drain);
    done.send(()).unwrap();
}

#[test]
#[ignore = "requires hardware D3D11"]
fn hardware_renderer_stage_markers_require_ordered_complete_scopes_and_reset_on_reuse() {
    let adapter = adapters()
        .unwrap()
        .into_iter()
        .find(|adapter| !adapter.software)
        .unwrap()
        .id;
    let device = Device::new(adapter).unwrap();
    let texture = Texture::new(
        &device.device,
        TextureDescriptor {
            extent: Extent2D::new(8, 8).unwrap(),
            format: PixelFormat::Rgba8Unorm,
            color_encoding: ColorEncoding::Linear,
            alpha_mode: AlphaMode::Premultiplied,
        },
    )
    .unwrap();
    let recorder = Recorder::new(DiagnosticsConfig {
        capacity: NonZeroUsize::new(8).unwrap(),
    });
    let mut timing = GpuTiming::new(&device.device, recorder.clone(), DiagnosticContext::Renderer);
    let mut completion = EventCompletion::new(&device).unwrap();
    let generation = DeviceGeneration::new(Uuid::new_v4(), 1);
    let mut drain = Drain {
        device: &device,
        completion: &mut completion,
        timing: &mut timing,
        generation,
        submitted: 0,
    };
    for serial in 1..=5 {
        let token = CompletionToken {
            device: generation,
            serial,
        };
        drain.submitted = serial;
        {
            let _completion = drain.completion.begin(&device, token);
            let mut scope = drain.timing.begin(&device.context, token);
            // SAFETY: finite owned texture on this same context, between prepared timestamps.
            unsafe {
                device
                    .context
                    .ClearRenderTargetView(&texture.rtv, &[0.1, 0.2, 0.3, 1.0])
            };
            match serial {
                1 | 5 => {
                    scope.inputs_complete();
                    scope.render_complete();
                }
                2 => scope.inputs_complete(),
                3 => {
                    scope.render_complete();
                    scope.inputs_complete();
                }
                4 => {
                    scope.inputs_complete();
                    scope.inputs_complete();
                    scope.render_complete();
                }
                _ => unreachable!(),
            }
        }
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let completed = drain.completion.completed(&device, generation).unwrap();
            drain.timing.poll(&device.context, completed);
            if completed.serial == serial && recorder.snapshot().written == serial {
                break;
            }
            assert!(Instant::now() < deadline, "finite stage query completion");
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    let mut rows = Vec::new();
    recorder.drain(&mut rows, NonZeroUsize::new(8).unwrap());
    assert_eq!(rows.len(), 5);
    for event in rows {
        let DiagnosticEvent::Gpu(row) = event else {
            panic!("expected GPU row")
        };
        assert_eq!(row.status, GpuSampleStatus::Valid);
        assert_eq!(row.token.device, generation);
        if matches!(row.token.serial, 1 | 5) {
            let inputs = row.renderer_inputs_end.expect("complete input stage");
            let render = row.renderer_render_end.expect("complete rendering stage");
            assert!(row.begin.unwrap() <= inputs && inputs <= render && render <= row.end.unwrap());
            assert_eq!(
                (inputs - row.begin.unwrap()) + (render - inputs) + (row.end.unwrap() - render),
                row.end.unwrap() - row.begin.unwrap()
            );
        } else {
            assert_eq!((row.renderer_inputs_end, row.renderer_render_end), (None, None));
        }
    }
    assert_eq!(recorder.snapshot().dropped_full, 0);
}

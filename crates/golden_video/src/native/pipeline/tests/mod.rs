use super::*;
mod mapping;
use std::num::NonZeroUsize;
use std::time::{Duration, Instant};

use crate::native::completion::EventCompletion;
use crate::native::device::{Device, adapters};
use crate::native::timing::{
    DiagnosticContext, DiagnosticEvent, DiagnosticsConfig, GpuSampleStatus, GpuTiming, Recorder,
};
use crate::runtime::{CompletionToken, DeviceGeneration};
use crate::{Extent2D, PixelFormat, TextureDescriptor};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM_SRGB;

#[test]
#[ignore = "requires an explicitly available Windows hardware D3D11 adapter"]
fn hardware_opaque_transfer_branch_cost_control() {
    let adapter = adapters()
        .unwrap()
        .into_iter()
        .find(|adapter| !adapter.software)
        .unwrap();
    let gpu = Device::new(adapter.id).unwrap();
    let mut pipelines = [Pipeline::new(&gpu.device).unwrap(), Pipeline::new(&gpu.device).unwrap()];
    assert_eq!(SHADER.matches("[branch]").count(), 1);
    let flattened = SHADER.replace("[branch]", "[flatten]");
    let mut code = None;
    let mut pixel = None;
    unsafe {
        D3DCompile(
            flattened.as_ptr().cast(),
            flattened.len(),
            s!("flattened-output-control"),
            None,
            None,
            s!("ps_main"),
            s!("ps_5_0"),
            0,
            0,
            &mut code,
            None,
        )
        .unwrap();
        gpu.device
            .CreatePixelShader(blob(code.as_ref().unwrap()), None, Some(&mut pixel))
            .unwrap();
    }
    pipelines[0].pixel = pixel.unwrap();
    let source_desc = TextureDescriptor {
        extent: Extent2D::new(3840, 2160).unwrap(),
        format: PixelFormat::Rgba8Unorm,
        color_encoding: ColorEncoding::Linear,
        alpha_mode: AlphaMode::Premultiplied,
    };
    let source = Texture::new(&gpu.device, source_desc).unwrap();
    let target_desc = TextureDescriptor {
        format: PixelFormat::Bgra8Unorm,
        color_encoding: ColorEncoding::Srgb,
        alpha_mode: AlphaMode::Opaque,
        ..source_desc
    };
    // A finite private target models the production window's hardware transfer view without
    // Present/compositor/SDK work. It is never exposed as a Spout wire image.
    let storage = Texture::new_sampled(&gpu.device, target_desc).unwrap();
    let mut view = None;
    unsafe {
        gpu.device
            .CreateRenderTargetView(
                &storage.texture,
                Some(&D3D11_RENDER_TARGET_VIEW_DESC {
                    Format: DXGI_FORMAT_B8G8R8A8_UNORM_SRGB,
                    ViewDimension: D3D11_RTV_DIMENSION_TEXTURE2D,
                    Anonymous: D3D11_RENDER_TARGET_VIEW_DESC_0 {
                        Texture2D: D3D11_TEX2D_RTV { MipSlice: 0 },
                    },
                }),
                Some(&mut view),
            )
            .unwrap();
        gpu.context.ClearRenderTargetView(&source.rtv, &[0.18, 0.57, 0.83, 1.0]);
    }
    let view = view.unwrap();
    let target = RenderTarget {
        view: &view,
        descriptor: target_desc,
        shader_encoding: ColorEncoding::Linear,
    };
    let recorder = Recorder::new(DiagnosticsConfig {
        capacity: NonZeroUsize::new(4).unwrap(),
    });
    let mut timing = GpuTiming::new(&gpu.device, recorder.clone(), DiagnosticContext::Renderer);
    let mut completion = EventCompletion::new(&gpu).unwrap();
    let generation = DeviceGeneration::new(uuid::Uuid::new_v4(), 1);
    let mut samples = [Vec::new(), Vec::new()];
    let mut rows = Vec::new();
    let mut serial = 0;
    for pair in 0..16 {
        for index in if pair % 2 == 0 { [0, 1] } else { [1, 0] } {
            serial += 1;
            let token = CompletionToken {
                device: generation,
                serial,
            };
            {
                let _completion = completion.begin(&gpu, token);
                let _timing = timing.begin(&gpu.context, token);
                for _ in 0..4 {
                    pipelines[index].draw(
                        &gpu.context,
                        &source,
                        None,
                        target,
                        1.0,
                        BlendMode::Normal,
                        SampleFilter::Nearest,
                    );
                }
            }
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                let done = completion.completed(&gpu, generation).unwrap();
                timing.poll(&gpu.context, done);
                recorder.drain(&mut rows, NonZeroUsize::new(4).unwrap());
                if done.serial == serial && !rows.is_empty() {
                    break;
                }
                assert!(Instant::now() < deadline, "finite output shader GPU drain");
                std::thread::sleep(Duration::from_millis(1));
            }
            assert_eq!(rows.len(), 1);
            let DiagnosticEvent::Gpu(row) = rows.pop().unwrap() else {
                panic!("expected GPU timing")
            };
            assert_eq!(row.token, token);
            assert_eq!(row.status, GpuSampleStatus::Valid);
            if pair >= 4 {
                samples[index].push(
                    (row.end.unwrap() - row.begin.unwrap()) as f64 * 1000.0 / row.frequency.unwrap() as f64 / 4.0,
                );
            }
        }
    }
    assert_eq!(recorder.snapshot().dropped_full, 0);
    unsafe { gpu.context.ClearState() };
    eprintln!(
        "OUTPUT_TRANSFER_TIMING {}",
        serde_json::json!({
            "adapter": adapter.id, "extent": [3840,2160], "warmup_pairs": 4,
            "measured_pairs": 12, "draws_per_batch": 4,
            "flattened_gpu_ms_per_draw": samples[0], "branched_gpu_ms_per_draw": samples[1],
            "scope": "isolated opaque draw to hardware sRGB target; no Present/SDK work or end-to-end qualification"
        })
    );
}

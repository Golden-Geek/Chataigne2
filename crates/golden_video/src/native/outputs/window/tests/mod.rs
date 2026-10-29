use std::time::{Duration, Instant};

use uuid::Uuid;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::w;

use super::*;
use crate::native::backend::D3d11Backend;
use crate::native::completion::EventCompletion;
use crate::native::device::{Device, adapters};
use crate::native::pipeline::{CONSTANT_BYTES, Pipeline};
use crate::native::texture::Texture;
use crate::native::{D3d11Config, D3d11Control, InputLimits, OutputBinding};
use crate::plan::{BlendMode, PlanLimits, PlanOutput, RenderOperation, RenderPass, RenderPlan, SampleFilter};
use crate::runtime::backend::Backend;
use crate::runtime::{CompletionToken, DeviceGeneration};
use crate::{AlphaMode, Extent2D, LinearColor, OutputId, PassId, PixelFormat, TextureId};

struct OwnedWindow(HWND);
impl OwnedWindow {
    fn new() -> Self {
        // SAFETY: built-in window class, no borrowed creation data. This test owns destruction.
        Self(
            unsafe {
                CreateWindowExW(
                    WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
                    w!("STATIC"),
                    w!("Golden window pixel test"),
                    WS_POPUP | WS_VISIBLE,
                    30,
                    30,
                    64,
                    64,
                    None,
                    None,
                    None,
                    None,
                )
            }
            .unwrap(),
        )
    }
    fn target(&self) -> WindowTarget {
        // SAFETY: the guard outlives this test's endpoint and its actual GPU retirement.
        unsafe { WindowTarget::from_raw(self.0.0 as isize) }.unwrap()
    }
}
impl Drop for OwnedWindow {
    fn drop(&mut self) {
        // SAFETY: only this guard owns the test HWND, on its creating thread.
        unsafe { DestroyWindow(self.0) }.unwrap();
    }
}

fn pump() {
    let mut message = MSG::default();
    // SAFETY: this test thread's message queue and initialized message storage.
    unsafe {
        while PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

fn descriptor() -> TextureDescriptor {
    TextureDescriptor {
        extent: Extent2D::new(4, 4).unwrap(),
        format: PixelFormat::Bgra8Unorm,
        color_encoding: ColorEncoding::Srgb,
        alpha_mode: AlphaMode::Opaque,
    }
}

struct Drain<'a> {
    completion: &'a mut EventCompletion,
    device: &'a Device,
    last: CompletionToken,
}
impl Drop for Drain<'_> {
    fn drop(&mut self) {
        while !self.device.removed()
            && !self
                .completion
                .completed(self.device, self.last.device)
                .is_ok_and(|done| done.serial >= self.last.serial)
        {
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}

#[test]
#[ignore = "requires Windows hardware D3D11 and an owned native window"]
fn hardware_direct_window_encodes_once_and_tracks_flip_buffers_across_presents() {
    let owned = OwnedWindow::new();
    let adapter = adapters()
        .unwrap()
        .into_iter()
        .find(|adapter| !adapter.software)
        .unwrap();
    eprintln!(
        "direct window pixel hardware LUID {:08x}:{:08x}",
        adapter.id.high as u32, adapter.id.low
    );
    let device = Device::new(adapter.id).unwrap();
    let mut window = WindowOutput::new(&device.device, owned.target(), descriptor(), false).unwrap();
    let source = Texture::new(
        &device.device,
        TextureDescriptor {
            format: PixelFormat::Rgba8Unorm,
            color_encoding: ColorEncoding::Linear,
            alpha_mode: AlphaMode::Premultiplied,
            ..descriptor()
        },
    )
    .unwrap();
    let copied = Texture::new(&device.device, descriptor()).unwrap();
    let pipeline = Pipeline::new(&device.device).unwrap();
    let mut completion = EventCompletion::new(&device).unwrap();
    let generation = DeviceGeneration::new(Uuid::new_v4(), 1);
    let mut drain = Drain {
        completion: &mut completion,
        device: &device,
        last: CompletionToken {
            device: generation,
            serial: 0,
        },
    };
    let colors: [[u8; 4]; 6] = [
        [0, 0, 0, 0],
        [1, 1, 1, 255],
        [3, 2, 1, 128],
        [40, 15, 70, 128],
        [255, 255, 255, 255],
        [8, 61, 126, 192],
    ];
    for (index, color) in colors.into_iter().enumerate() {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            pump();
            match window.admit() {
                Ok(()) => break,
                Err(Delivery::Busy) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(1)),
                other => panic!("window admission: {other:?}"),
            }
        }
        drain.last.serial = index as u64 + 1;
        {
            let _submission = drain.completion.begin(&device, drain.last);
            let values = color.map(|v| f32::from(v) / 255.0);
            // SAFETY: finite test-owned source storage; the production pipeline consumes it.
            unsafe { device.context.ClearRenderTargetView(&source.rtv, &values) };
            pipeline.draw(
                &device.context,
                &source,
                None,
                window.render_target(),
                1.0,
                BlendMode::Normal,
                SampleFilter::Nearest,
            );
            // SAFETY: finite test-only copy before Present, from the actual buffer-zero resource.
            unsafe {
                let buffer: ID3D11Texture2D = window.chain.GetBuffer(0).unwrap();
                device.context.CopyResource(&copied.texture, &buffer);
            }
            let actual = crate::native::tests::pixels::readback(&device, &copied);
            let expected = color.map(|v| f64::from(v) / 255.0);
            for pixel in actual {
                for channel in 0..3 {
                    let value = expected[channel];
                    let encoded = if value <= 0.0031308 {
                        value * 12.92
                    } else {
                        1.055 * value.powf(1.0 / 2.4) - 0.055
                    };
                    assert!(
                        (f64::from(pixel[channel]) / 255.0 - encoded).abs() <= 2.0 / 255.0,
                        "frame {index}: {pixel:?} vs encoded {color:?}"
                    );
                }
                assert_eq!(pixel[3], 255, "opaque windows composite coverage over black");
            }
            loop {
                match window.present(None, &mut None) {
                    Delivery::Accepted => break,
                    Delivery::Busy if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(1)),
                    other => panic!("window Present: {other:?}"),
                }
            }
        }
        while drain.completion.completed(&device, generation).unwrap().serial < drain.last.serial {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    eprintln!("direct window: six admitted flip-buffer frames, all 16 pixels/frame within 2/255");
}

#[test]
#[ignore = "requires Windows hardware D3D11 and an owned native window"]
fn hardware_window_charge_has_only_two_backbuffers_and_rejects_foreign_context_before_drawing() {
    let owned = OwnedWindow::new();
    let adapter = adapters()
        .unwrap()
        .into_iter()
        .find(|adapter| !adapter.software)
        .unwrap()
        .id;
    let generation = DeviceGeneration::new(Uuid::new_v4(), 1);
    let control = D3d11Control::default();
    let config = D3d11Config {
        diagnostics: None,
        adapter,
        inputs: InputLimits::default(),
        outputs: vec![OutputBinding::Window {
            id: OutputId::new(1).unwrap(),
            window: owned.target(),
            extent: descriptor().extent,
        }],
    };
    let mut backend = D3d11Backend::new(config.clone(), control.clone(), generation);
    let plan = RenderPlan {
        external_textures: vec![],
        passes: vec![RenderPass {
            id: PassId::new(1).unwrap(),
            output: TextureId::new(1).unwrap(),
            descriptor: TextureDescriptor {
                format: PixelFormat::Rgba8Unorm,
                color_encoding: ColorEncoding::Linear,
                alpha_mode: AlphaMode::Premultiplied,
                ..descriptor()
            },
            operation: RenderOperation::Clear(LinearColor::TRANSPARENT),
        }],
        outputs: vec![PlanOutput {
            id: OutputId::new(1).unwrap(),
            source: TextureId::new(1).unwrap(),
        }],
    }
    .compile(&PlanLimits {
        max_passes: 2,
        max_external_textures: 0,
        max_outputs: 1,
        max_dependencies: 2,
        max_texture_dimension: 16,
        max_texture_bytes: 1024,
    })
    .unwrap();
    assert_eq!(backend.base_resource_bytes().unwrap(), CONSTANT_BYTES + 2 * 64);
    assert_eq!(
        backend.resource_bytes(&plan).unwrap(),
        64,
        "no generation-owned window conversion texture"
    );
    backend.initialize().unwrap();
    let resources = backend
        .prepare(
            &plan,
            crate::runtime::GenerationInfo {
                generation: crate::runtime::GenerationId::new(1),
                identity: crate::runtime::PlanIdentity {
                    project: crate::runtime::ProjectEpoch::new(Uuid::new_v4(), 0),
                    revision: crate::runtime::PlanRevision::new(1),
                },
                device: generation,
                bytes: 64,
                output_count: 1,
            },
        )
        .unwrap();
    assert_eq!(
        control.snapshot().created_textures,
        1,
        "only the render-pass texture is allocated"
    );
    drop(resources);
    drop(backend);
    let another = OwnedWindow::new();
    let mut config = config;
    config.outputs = vec![OutputBinding::Window {
        id: OutputId::new(1).unwrap(),
        window: another.target(),
        extent: descriptor().extent,
    }];
    let device = Device::new(adapter).unwrap();
    let mut pool = crate::native::outputs::OutputPool::new(&device.device, config, control, generation).unwrap();
    let foreign = Device::new(adapter).unwrap();
    let result = pool.admit(
        OutputId::new(1).unwrap(),
        &foreign.context,
        descriptor(),
        CompletionToken {
            device: generation,
            serial: 1,
        },
    );
    assert!(matches!(result, Err(Delivery::Failed(_))));
    drop(result);
    assert!(!pool.needs_conversion_texture(OutputId::new(1).unwrap()));
}

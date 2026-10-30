use std::time::{Duration, Instant};

use windows::Win32::Foundation::S_OK;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::DXGI_ERROR_WAS_STILL_DRAWING;
use windows::core::Interface;

use crate::plan::{BlendMode, SampleFilter};
use crate::{AlphaMode, ColorEncoding, Extent2D, PixelFormat, TextureDescriptor};

use super::super::device::{Device, adapters};
use super::super::pipeline::Pipeline;
use super::super::texture::Texture;

const TOLERANCE: f64 = 2.0 / 255.0;

#[test]
#[ignore = "requires an explicitly available Windows hardware D3D11 adapter"]
fn hardware_pipeline_matches_finite_color_alpha_filter_and_blend_oracles() {
    let fixture = Fixture::new();

    let srgb = fixture.upload(
        descriptor(2, PixelFormat::Bgra8Unorm, ColorEncoding::Srgb, AlphaMode::Opaque),
        &[[0, 0, 0, 255], [255, 255, 255, 255]],
    );
    let filtered = fixture.target(internal(PixelFormat::Rgba8Unorm));
    fixture.draw(&srgb, None, &filtered, 1.0, BlendMode::Normal, SampleFilter::Linear);
    fixture.check(&filtered, [0.5, 0.5, 0.5, 1.0], "sRGB decode precedes interpolation");

    let straight = fixture.upload(
        descriptor(2, PixelFormat::Rgba8Unorm, ColorEncoding::Srgb, AlphaMode::Straight),
        &[[255, 0, 0, 0], [0, 255, 0, 255]],
    );
    fixture.draw(&straight, None, &filtered, 1.0, BlendMode::Normal, SampleFilter::Linear);
    fixture.check(
        &filtered,
        [0.0, 0.5, 0.0, 0.5],
        "coverage premultiplication precedes interpolation",
    );

    let premultiplied = [64, 32, 96, 128];
    let encoded = fixture.upload(
        descriptor(
            1,
            PixelFormat::Bgra8Unorm,
            ColorEncoding::Srgb,
            AlphaMode::Premultiplied,
        ),
        &[premultiplied],
    );
    fixture.draw(&encoded, None, &filtered, 1.0, BlendMode::Normal, SampleFilter::Nearest);
    let coverage = f64::from(premultiplied[3]) / 255.0;
    let mut decoded = [0.0; 4];
    for channel in 0..3 {
        decoded[channel] = decode(f64::from(premultiplied[channel]) / 255.0 / coverage) * coverage;
    }
    decoded[3] = coverage;
    fixture.check(&filtered, decoded, "encoded premultiplied source conversion");

    let source_pixel = [31, 54, 16, 96];
    let destination_pixel = [43, 11, 83, 160];
    let source = fixture.upload(internal(PixelFormat::Rgba8Unorm), &[source_pixel]);
    let destination = fixture.upload(internal(PixelFormat::Bgra8Unorm), &[destination_pixel]);
    let opacity = 0.6_f32;
    for mode in [
        BlendMode::Normal,
        BlendMode::Add,
        BlendMode::Multiply,
        BlendMode::Screen,
    ] {
        fixture.draw(
            &source,
            Some(&destination),
            &filtered,
            opacity,
            mode,
            SampleFilter::Nearest,
        );
        fixture.check(
            &filtered,
            reference_blend(source_pixel, destination_pixel, f64::from(opacity), mode),
            &format!("fractional-alpha {mode:?}"),
        );
    }

    let linear_pixel = [40, 15, 70, 128];
    let linear = fixture.upload(internal(PixelFormat::Rgba8Unorm), &[linear_pixel]);
    let opaque = fixture.target(descriptor(
        1,
        PixelFormat::Bgra8Unorm,
        ColorEncoding::Srgb,
        AlphaMode::Opaque,
    ));
    fixture.draw(&linear, None, &opaque, 1.0, BlendMode::Normal, SampleFilter::Nearest);
    fixture.check(
        &opaque,
        [encode(40.0 / 255.0), encode(15.0 / 255.0), encode(70.0 / 255.0), 1.0],
        "opaque sRGB output composites linear coverage over black",
    );

    let encoded_output = fixture.target(descriptor(
        1,
        PixelFormat::Bgra8Unorm,
        ColorEncoding::Srgb,
        AlphaMode::Premultiplied,
    ));
    fixture.draw(
        &linear,
        None,
        &encoded_output,
        1.0,
        BlendMode::Normal,
        SampleFilter::Nearest,
    );
    let alpha = f64::from(linear_pixel[3]) / 255.0;
    fixture.check(
        &encoded_output,
        [
            encode(40.0 / 255.0 / alpha) * alpha,
            encode(15.0 / 255.0 / alpha) * alpha,
            encode(70.0 / 255.0 / alpha) * alpha,
            alpha,
        ],
        "sRGB output encodes straight color before premultiplication",
    );
}

#[test]
#[ignore = "requires an explicitly available Windows hardware D3D11 adapter"]
fn hardware_source_views_preserve_srgb_and_alpha_before_filtering() {
    let fixture = Fixture::new();
    let target = fixture.target(internal(PixelFormat::Rgba8Unorm));
    // Nontrivial transfer values and transparent colored storage detect double decoding,
    // interpolation in encoded space, and premultiplication in the wrong order.
    let pixels = [[46, 182, 247, 0], [117, 53, 37, 128]];
    for format in [PixelFormat::Rgba8Unorm, PixelFormat::Bgra8Unorm] {
        for alpha in [AlphaMode::Straight, AlphaMode::Opaque, AlphaMode::Premultiplied] {
            let desc = descriptor(2, format, ColorEncoding::Srgb, alpha);
            let source = Texture::new_sampled(&fixture.gpu.device, desc).expect("prepare sampled source");
            let expected_encoding = if alpha == AlphaMode::Premultiplied {
                ColorEncoding::Srgb
            } else {
                ColorEncoding::Linear
            };
            assert_eq!(source.sampling_view().1, expected_encoding);
            fixture.write(&source, &pixels);
            let decode_pixel = |pixel: [u8; 4]| {
                let coverage = if alpha == AlphaMode::Opaque {
                    1.0
                } else {
                    f64::from(pixel[3]) / 255.0
                };
                let mut result = [0.0; 4];
                if coverage > 0.0 {
                    for channel in 0..3 {
                        let stored = f64::from(pixel[channel]) / 255.0;
                        let straight = if alpha == AlphaMode::Premultiplied {
                            stored / coverage
                        } else {
                            stored
                        };
                        result[channel] = decode(straight) * coverage;
                    }
                    result[3] = coverage;
                }
                result
            };
            let left = decode_pixel(pixels[0]);
            let right = decode_pixel(pixels[1]);
            let expected = std::array::from_fn(|channel| (left[channel] + right[channel]) * 0.5);
            fixture.draw(&source, None, &target, 1.0, BlendMode::Normal, SampleFilter::Linear);
            fixture.check(&target, expected, &format!("source view {format:?}/{alpha:?}"));
            fixture.draw(&source, None, &target, 1.0, BlendMode::Normal, SampleFilter::Nearest);
            fixture.check(&target, right, "nearest source view decodes exactly once");
        }
        let linear = Texture::new_sampled(
            &fixture.gpu.device,
            descriptor(2, format, ColorEncoding::Linear, AlphaMode::Straight),
        )
        .expect("prepare linear source");
        assert_eq!(linear.sampling_view().1, ColorEncoding::Linear);
        fixture.write(&linear, &pixels);
        fixture.draw(&linear, None, &target, 1.0, BlendMode::Normal, SampleFilter::Linear);
        let coverage = 128.0 / 255.0;
        fixture.check(
            &target,
            [
                117.0 / 255.0 * coverage / 2.0,
                53.0 / 255.0 * coverage / 2.0,
                37.0 / 255.0 * coverage / 2.0,
                coverage / 2.0,
            ],
            "linear sources retain their transfer",
        );
    }
}

#[test]
#[ignore = "requires an explicitly available Windows hardware D3D11 adapter"]
fn hardware_srgb_source_decode_cost_control() {
    use std::num::NonZeroUsize;
    use uuid::Uuid;

    use crate::native::completion::EventCompletion;
    use crate::native::timing::{
        DiagnosticContext, DiagnosticEvent, DiagnosticsConfig, GpuSampleStatus, GpuTiming, Recorder,
    };
    use crate::runtime::{CompletionToken, DeviceGeneration};

    let fixture = Fixture::new();
    let source_desc = TextureDescriptor {
        extent: Extent2D::new(1920, 1080).unwrap(),
        format: PixelFormat::Bgra8Unorm,
        color_encoding: ColorEncoding::Srgb,
        alpha_mode: AlphaMode::Straight,
    };
    let sources = [
        fixture.target(source_desc),
        Texture::new_sampled(&fixture.gpu.device, source_desc).unwrap(),
    ];
    let target = fixture.target(TextureDescriptor {
        extent: Extent2D::new(3840, 2160).unwrap(),
        ..internal(PixelFormat::Rgba8Unorm)
    });
    for source in &sources {
        // SAFETY: finite test-owned raw views; both paths receive identical encoded texels.
        unsafe {
            fixture
                .gpu
                .context
                .ClearRenderTargetView(&source.rtv, &[0.18, 0.57, 0.83, 0.63])
        };
    }
    let recorder = Recorder::new(DiagnosticsConfig {
        capacity: NonZeroUsize::new(4).unwrap(),
    });
    let mut timing = GpuTiming::new(&fixture.gpu.device, recorder.clone(), DiagnosticContext::Renderer);
    let mut completion = EventCompletion::new(&fixture.gpu).unwrap();
    let generation = DeviceGeneration::new(Uuid::new_v4(), 1);
    let mut measurements = [Vec::new(), Vec::new()];
    let mut rows = Vec::new();
    let mut serial = 0;
    for pair in 0..16 {
        // Alternate order to avoid consistently favoring one path after idle/warmup.
        for index in if pair % 2 == 0 { [0, 1] } else { [1, 0] } {
            serial += 1;
            let token = CompletionToken {
                device: generation,
                serial,
            };
            {
                let _completion = completion.begin(&fixture.gpu, token);
                let _timing = timing.begin(&fixture.gpu.context, token);
                for _ in 0..4 {
                    fixture.draw(
                        &sources[index],
                        None,
                        &target,
                        1.0,
                        BlendMode::Normal,
                        SampleFilter::Linear,
                    );
                }
            }
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                let done = completion.completed(&fixture.gpu, generation).unwrap();
                timing.poll(&fixture.gpu.context, done);
                recorder.drain(&mut rows, NonZeroUsize::new(4).unwrap());
                if done.serial == serial && !rows.is_empty() {
                    break;
                }
                assert!(Instant::now() < deadline, "finite sampler GPU timing drain");
                std::thread::sleep(Duration::from_millis(1));
            }
            assert_eq!(rows.len(), 1);
            let DiagnosticEvent::Gpu(row) = rows.pop().unwrap() else {
                panic!("expected GPU timing")
            };
            assert_eq!(row.token, token);
            assert_eq!(row.status, GpuSampleStatus::Valid);
            if pair >= 4 {
                measurements[index].push(
                    (row.end.unwrap() - row.begin.unwrap()) as f64 * 1000.0 / row.frequency.unwrap() as f64 / 4.0,
                );
            }
        }
    }
    assert_eq!(recorder.snapshot().dropped_full, 0);
    eprintln!(
        "SAMPLER_TIMING {}",
        serde_json::json!({
            "warmup_pairs": 4,
            "measured_pairs": 12,
            "draws_per_batch": 4,
            "source": [1920, 1080],
            "target": [3840, 2160],
            "manual_decode_gpu_ms_per_draw": measurements[0],
            "hardware_decode_gpu_ms_per_draw": measurements[1],
            "scope": "isolated sampler cost; not an end-to-end renderer critical path",
        })
    );
}

#[test]
#[ignore = "requires an explicitly available Windows hardware D3D11 adapter"]
fn hardware_output_transfer_branch_matches_color_and_alpha_oracles() {
    let fixture = Fixture::new();
    let source = fixture.target(internal(PixelFormat::Rgba8Unorm));
    let pixels = [[0, 0, 0, 0], [1, 2, 3, 17], [10, 32, 65, 128], [255, 120, 255, 255]];
    for format in [PixelFormat::Rgba8Unorm, PixelFormat::Bgra8Unorm] {
        for encoding in [ColorEncoding::Linear, ColorEncoding::Srgb] {
            for alpha in [AlphaMode::Opaque, AlphaMode::Straight, AlphaMode::Premultiplied] {
                let target = fixture.target(descriptor(1, format, encoding, alpha));
                for pixel in pixels {
                    fixture.write(&source, &[pixel]);
                    for opacity in [1.0_f32, 0.125] {
                        fixture.draw(
                            &source,
                            None,
                            &target,
                            opacity,
                            BlendMode::Normal,
                            SampleFilter::Nearest,
                        );
                        let coverage = f64::from(pixel[3]) / 255.0 * f64::from(opacity);
                        let expected = std::array::from_fn(|channel| {
                            if channel == 3 {
                                return if alpha == AlphaMode::Opaque { 1.0 } else { coverage };
                            }
                            let linear = f64::from(pixel[channel]) / 255.0 * f64::from(opacity);
                            let straight = if alpha == AlphaMode::Opaque {
                                linear
                            } else if coverage > 0.0 {
                                linear / coverage
                            } else {
                                0.0
                            };
                            let transferred = if encoding == ColorEncoding::Srgb {
                                encode(straight)
                            } else {
                                straight
                            };
                            if alpha == AlphaMode::Premultiplied {
                                transferred * coverage
                            } else {
                                transferred
                            }
                        });
                        fixture.check(
                            &target,
                            expected,
                            &format!("output view {format:?}/{encoding:?}/{alpha:?}"),
                        );
                    }
                }
            }
        }
    }
}

struct Fixture {
    gpu: Device,
    pipeline: Pipeline,
}

impl Fixture {
    fn new() -> Self {
        let adapter = adapters()
            .expect("enumerate D3D11 adapters")
            .into_iter()
            .find(|adapter| !adapter.software)
            .expect("explicit hardware test requires a hardware adapter");
        eprintln!(
            "finite D3D11 pipeline pixels on hardware adapter LUID {:08x}:{:08x}",
            adapter.id.high as u32, adapter.id.low
        );
        let gpu = Device::new(adapter.id).expect("create exact selected hardware device");
        let pipeline = Pipeline::new(&gpu.device).expect("prepare production shaders/state");
        Self { gpu, pipeline }
    }

    fn target(&self, descriptor: TextureDescriptor) -> Texture {
        Texture::new(&self.gpu.device, descriptor).expect("prepare texture and views")
    }

    fn upload(&self, descriptor: TextureDescriptor, rgba: &[[u8; 4]]) -> Texture {
        let texture = self.target(descriptor);
        self.write(&texture, rgba);
        texture
    }

    fn write(&self, texture: &Texture, rgba: &[[u8; 4]]) {
        let descriptor = texture.descriptor;
        assert_eq!(descriptor.extent.height(), 1);
        assert!(descriptor.extent.width() <= 2);
        assert_eq!(rgba.len(), descriptor.extent.width() as usize);
        let storage = rgba
            .iter()
            .copied()
            .map(|pixel| storage_order(pixel, descriptor.format))
            .collect::<Vec<_>>();
        // SAFETY: finite test-only initialized pixel upload with the declared row layout.
        unsafe {
            self.gpu.context.UpdateSubresource(
                &texture.texture,
                0,
                None,
                storage.as_ptr().cast(),
                descriptor.extent.width() * 4,
                0,
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw(
        &self,
        source: &Texture,
        previous: Option<&Texture>,
        target: &Texture,
        opacity: f32,
        blend: BlendMode,
        filter: SampleFilter,
    ) {
        self.pipeline.draw(
            &self.gpu.context,
            source,
            previous,
            target.into(),
            opacity,
            blend,
            filter,
        );
    }

    fn check(&self, texture: &Texture, expected: [f64; 4], label: &str) {
        let pixels = readback(&self.gpu, texture);
        assert_eq!(pixels.len(), 1);
        let actual = pixels[0];
        for channel in 0..4 {
            let difference = (f64::from(actual[channel]) / 255.0 - expected[channel]).abs();
            assert!(
                difference <= TOLERANCE,
                "{label}, channel {channel}: {actual:?} vs {expected:?}, error {difference} > {TOLERANCE}"
            );
        }
        eprintln!("{label}: RGBA {actual:?}, reference {expected:?}, tolerance 2/255");
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        // SAFETY: the finite readbacks completed before fixture cleanup; release context-held state.
        unsafe { self.gpu.context.ClearState() };
    }
}

pub(in crate::native) fn readback(gpu: &Device, texture: &Texture) -> Vec<[u8; 4]> {
    let extent = texture.descriptor.extent;
    assert!(extent.width() <= 8 && extent.height() <= 8);
    let mut desc = D3D11_TEXTURE2D_DESC::default();
    // SAFETY: owned texture and writable descriptor.
    unsafe { texture.texture.GetDesc(&mut desc) };
    desc.Usage = D3D11_USAGE_STAGING;
    desc.BindFlags = 0;
    desc.MiscFlags = 0;
    desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
    let mut staging = None;
    let mut query = None;
    // SAFETY: bounded matching staging texture and owned event-query outputs.
    unsafe {
        gpu.device
            .CreateTexture2D(&desc, None, Some(&mut staging))
            .expect("prepare finite readback");
        gpu.device
            .CreateQuery(
                &D3D11_QUERY_DESC {
                    Query: D3D11_QUERY_EVENT,
                    MiscFlags: 0,
                },
                Some(&mut query),
            )
            .expect("prepare readback completion query");
    }
    let staging = staging.expect("missing staging texture");
    let query = query.expect("missing completion query");
    // SAFETY: same device/layout, followed by a completion event and explicit submission.
    unsafe {
        gpu.context.CopyResource(&staging, &texture.texture);
        gpu.context.End(&query);
        gpu.context.Flush();
    }
    let started = Instant::now();
    loop {
        let mut completed = 0_u32;
        // SAFETY: native generated vtable preserves S_FALSE readiness, unlike Result<()>.
        // D3D11_QUERY_EVENT writes exactly BOOL/u32. DONOTFLUSH avoids per-poll submission.
        let result = unsafe {
            (gpu.context.vtable().GetData)(
                gpu.context.as_raw(),
                query.as_raw(),
                (&mut completed as *mut u32).cast(),
                size_of::<u32>() as u32,
                D3D11_ASYNC_GETDATA_DONOTFLUSH.0 as u32,
            )
        };
        result.ok().expect("read finite GPU completion");
        if result == S_OK && completed != 0 {
            break;
        }
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "finite GPU readback completion timed out"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
    let mut map_busy = 0;
    loop {
        // SAFETY: staging belongs to this device. DO_NOT_WAIT keeps readiness explicit,
        // even if the driver reports Map busy after the event query completed.
        let result = unsafe {
            gpu.context.Map(
                &staging,
                0,
                D3D11_MAP_READ,
                D3D11_MAP_FLAG_DO_NOT_WAIT.0 as u32,
                Some(&mut mapped),
            )
        };
        match result {
            Ok(()) => break,
            Err(error)
                if error.code() == DXGI_ERROR_WAS_STILL_DRAWING && started.elapsed() < Duration::from_secs(5) =>
            {
                map_busy += 1;
                std::thread::sleep(Duration::from_millis(1));
            }
            Err(error) => panic!("finite GPU readback map failed or exceeded its five-second deadline: {error}"),
        }
    }
    if map_busy != 0 {
        eprintln!("finite readback Map busy polls={map_busy}");
    }
    if mapped.pData.is_null() || mapped.RowPitch < extent.width() * 4 {
        // SAFETY: successful map must always be unmapped, including an invalid-layout failure.
        unsafe { gpu.context.Unmap(&staging, 0) };
        panic!("invalid finite readback layout");
    }
    let width = extent.width() as usize;
    let height = extent.height() as usize;
    let mut pixels = vec![[0; 4]; width * height];
    // SAFETY: the matching staging descriptor supplies every mapped row. Copy only its
    // initialized pixels, respecting native row padding and the exact destination extent.
    unsafe {
        for row in 0..height {
            std::ptr::copy_nonoverlapping(
                mapped.pData.cast::<u8>().add(row * mapped.RowPitch as usize),
                pixels.as_mut_ptr().cast::<u8>().add(row * width * 4),
                width * 4,
            );
        }
        gpu.context.Unmap(&staging, 0);
    }
    for pixel in &mut pixels {
        *pixel = storage_order(*pixel, texture.descriptor.format);
    }
    pixels
}

fn descriptor(
    width: u32,
    format: PixelFormat,
    color_encoding: ColorEncoding,
    alpha_mode: AlphaMode,
) -> TextureDescriptor {
    TextureDescriptor {
        extent: Extent2D::new(width, 1).unwrap(),
        format,
        color_encoding,
        alpha_mode,
    }
}

fn internal(format: PixelFormat) -> TextureDescriptor {
    descriptor(1, format, ColorEncoding::Linear, AlphaMode::Premultiplied)
}

fn storage_order(rgba: [u8; 4], format: PixelFormat) -> [u8; 4] {
    match format {
        PixelFormat::Rgba8Unorm => rgba,
        PixelFormat::Bgra8Unorm => [rgba[2], rgba[1], rgba[0], rgba[3]],
    }
}

fn decode(value: f64) -> f64 {
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

fn encode(value: f64) -> f64 {
    if value <= 0.0031308 {
        value * 12.92
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    }
}

fn reference_blend(source: [u8; 4], destination: [u8; 4], opacity: f64, mode: BlendMode) -> [f64; 4] {
    let source = source.map(|component| f64::from(component) / 255.0 * opacity);
    let destination = destination.map(|component| f64::from(component) / 255.0);
    if mode == BlendMode::Add {
        return std::array::from_fn(|channel| (source[channel] + destination[channel]).min(1.0));
    }
    let mut result = [0.0; 4];
    // Independent straight-color blend followed by source-over coverage, rather than the
    // shader's algebraically simplified premultiplied expressions.
    for channel in 0..3 {
        let foreground = source[channel] / source[3];
        let background = destination[channel] / destination[3];
        let blended = match mode {
            BlendMode::Normal => foreground,
            BlendMode::Multiply => foreground * background,
            BlendMode::Screen => 1.0 - (1.0 - foreground) * (1.0 - background),
            BlendMode::Add => unreachable!(),
        };
        result[channel] = (1.0 - source[3]) * destination[channel]
            + (1.0 - destination[3]) * source[channel]
            + source[3] * destination[3] * blended;
    }
    result[3] = 1.0 - (1.0 - source[3]) * (1.0 - destination[3]);
    result
}

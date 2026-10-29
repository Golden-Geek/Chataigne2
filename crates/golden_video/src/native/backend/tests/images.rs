use std::time::{Duration, Instant};

use golden_time::{PlaybackRate, ShowPosition, SystemClock, TimeAuthority};
use uuid::Uuid;

use super::super::*;
use crate::image::tests::{PreparedImageFixture, prepared_image};
use crate::native::{AdapterId, InputLimits, OutputBinding, adapters};
use crate::plan::{ExternalSource, ExternalTexture, PlanLimits, PlanOutput, RenderPass, RenderPlan};
use crate::{AlphaMode, ColorEncoding, Extent2D, PixelFormat};

const WAIT: Duration = Duration::from_secs(5);

fn descriptor(width: u32, encoding: ColorEncoding, alpha: AlphaMode) -> TextureDescriptor {
    TextureDescriptor {
        extent: Extent2D::new(width, 1).unwrap(),
        format: PixelFormat::Rgba8Unorm,
        color_encoding: encoding,
        alpha_mode: alpha,
    }
}

fn scene(image: &PreparedImageFixture, width: u32) -> CompiledPlan {
    RenderPlan {
        external_textures: vec![ExternalTexture {
            id: TextureId::new(1).unwrap(),
            descriptor: image.asset.value().descriptor(),
            source: Some(ExternalSource::Image(image.asset.clone())),
        }],
        passes: vec![RenderPass {
            id: PassId::new(1).unwrap(),
            output: TextureId::new(2).unwrap(),
            descriptor: descriptor(width, ColorEncoding::Linear, AlphaMode::Premultiplied),
            operation: RenderOperation::Sample {
                source: TextureId::new(1).unwrap(),
                filter: SampleFilter::Linear,
            },
        }],
        outputs: vec![PlanOutput {
            id: OutputId::new(1).unwrap(),
            source: TextureId::new(2).unwrap(),
        }],
    }
    .compile(&PlanLimits {
        max_passes: 4,
        max_external_textures: 4,
        max_outputs: 4,
        max_dependencies: 16,
        max_texture_dimension: 1024,
        max_texture_bytes: 65536,
    })
    .unwrap()
}

struct Renderer {
    resources: Option<D3d11Resources>,
    backend: D3d11Backend,
    plan: CompiledPlan,
    device: DeviceGeneration,
    serial: u64,
}

impl Renderer {
    fn new(adapter: AdapterId, image: &PreparedImageFixture, width: u32) -> Self {
        let config = D3d11Config {
            diagnostics: None,
            adapter,
            inputs: InputLimits {
                max_sources: 0,
                max_storage_bytes: 0,
            },
            outputs: vec![OutputBinding::Spout {
                id: OutputId::new(1).unwrap(),
                name: format!("GoldenImage-{}", Uuid::new_v4()),
                descriptor: descriptor(width, ColorEncoding::Linear, AlphaMode::Premultiplied),
            }],
        };
        let device = DeviceGeneration::new(Uuid::new_v4(), 1);
        let mut backend = D3d11Backend::new(config, Default::default(), device);
        backend.initialize().unwrap();
        let plan = scene(image, width);
        assert_eq!(plan.footprint().external_texture_bytes, 8);
        assert_eq!(backend.resource_bytes(&plan).unwrap(), 8 + u64::from(width) * 8);
        let resources = backend.prepare(&plan, device).unwrap();
        assert!(
            backend.control.sources().is_empty(),
            "image uploads must not acquire Spout receiver slots"
        );
        assert_eq!(backend.control.snapshot().uploaded_images, 1);
        assert_eq!(backend.control.snapshot().uploaded_image_bytes, 8);
        Self {
            resources: Some(resources),
            backend,
            plan,
            device,
            serial: 0,
        }
    }

    fn pixel_frame(&mut self) -> Vec<[u8; 4]> {
        self.serial += 1;
        let time = TimeAuthority::new(SystemClock::new(), ShowPosition::from_nanos(0), PlaybackRate::NORMAL)
            .unwrap()
            .sample()
            .unwrap();
        self.backend
            .submit(
                self.resources.as_ref().unwrap(),
                &self.plan,
                CompletionToken {
                    device: self.device,
                    serial: self.serial,
                },
                time,
            )
            .unwrap();
        let deadline = Instant::now() + WAIT;
        while self.backend.completion().unwrap().serial < self.serial {
            assert!(Instant::now() < deadline, "image frame completion timeout");
            std::thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(
            self.backend.control.snapshot().uploaded_images,
            1,
            "immutable image was uploaded per frame"
        );
        crate::native::tests::pixels::readback(
            &self.backend.native.as_ref().unwrap().device,
            &self.resources.as_ref().unwrap().outputs[&OutputId::new(1).unwrap()],
        )
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        self.backend.quiesce();
        while !self
            .backend
            .native
            .as_ref()
            .is_none_or(|native| native.device.removed())
        {
            if self.backend.completion().is_ok_and(|done| done.serial >= self.serial) {
                break;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        // Test readbacks already completed each submitted batch. No live use survives release.
        self.resources.take();
    }
}

fn expect(actual: &[[u8; 4]], expected: &[[u8; 4]]) {
    assert_eq!(actual.len(), expected.len());
    for (actual, expected) in actual.iter().zip(expected) {
        assert!(
            actual
                .iter()
                .zip(expected)
                .all(|(a, b)| (i32::from(*a) - i32::from(*b)).abs() <= 2),
            "uploaded image pixels {actual:?} differ from {expected:?}"
        );
    }
    eprintln!("immutable image pixels {actual:?}, expected {expected:?}, tolerance2/255");
}

#[test]
#[ignore = "requires explicitly available Windows hardware D3D11"]
fn hardware_immutable_image_upload_filters_alpha_once_and_recreates_from_the_retained_lease() {
    let (done, finished) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        if finished.recv_timeout(Duration::from_secs(60)).is_err() {
            eprintln!("immutable image hardware watchdog expired before verified retirement");
            std::process::exit(1);
        }
    });
    let adapter = adapters()
        .unwrap()
        .into_iter()
        .find(|adapter| !adapter.software)
        .unwrap();
    eprintln!(
        "immutable image hardware LUID {:08x}:{:08x}",
        adapter.id.high as u32, adapter.id.low
    );
    let image = prepared_image(
        descriptor(2, ColorEncoding::Srgb, AlphaMode::Straight),
        &[255, 0, 0, 0, 0, 255, 0, 255],
    );
    let retirement = image.retirement();
    let id = image.asset.id();
    let fingerprint = image.asset.fingerprint();
    let mut first = Renderer::new(adapter.id, &image, 1);
    let device = first.device;
    for _ in 0..3 {
        expect(&first.pixel_frame(), &[[0, 128, 0, 128]]);
    }
    drop(first);
    // Controlled completed teardown/new device, not an actual hardware-removal injection.
    let mut recreated = Renderer::new(adapter.id, &image, 2);
    assert_ne!(recreated.device, device);
    expect(&recreated.pixel_frame(), &[[0, 0, 0, 0], [0, 255, 0, 255]]);
    assert_eq!(image.asset.id(), id);
    assert_eq!(image.asset.fingerprint(), fingerprint);
    assert!(image.asset.is_current());
    image.binding.invalidate();
    assert!(!recreated.plan.admission_is_current());
    drop(recreated);
    drop(image);
    assert!(retirement.wait(WAIT));
    done.send(()).unwrap();
}

use std::time::{Duration, Instant};

use golden_time::{PlaybackRate, ShowPosition, SystemClock, TimeAuthority};
use uuid::Uuid;

use super::super::*;
use crate::image::tests::{PreparedImageFixture, prepared_image};
use crate::native::{AdapterId, InputLimits, OutputBinding, adapters};
use crate::plan::{
    BlitRegion, ExternalSource, ExternalTexture, PlanLimits, PlanOutput, RenderPass, RenderPlan, UnitRect,
};
use crate::{AlphaMode, ColorEncoding, Extent2D, PixelFormat};

const WAIT: Duration = Duration::from_secs(5);
const TRANSPARENT: [u8; 4] = [0; 4];
const GREEN: [u8; 4] = [0, 255, 0, 255];

fn descriptor(width: u32, height: u32) -> TextureDescriptor {
    TextureDescriptor {
        extent: Extent2D::new(width, height).unwrap(),
        format: PixelFormat::Rgba8Unorm,
        color_encoding: ColorEncoding::Linear,
        alpha_mode: AlphaMode::Premultiplied,
    }
}

fn region(source: u64, destination: UnitRect, source_rect: UnitRect) -> BlitRegion {
    BlitRegion {
        source: TextureId::new(source).unwrap(),
        destination,
        source_rect,
        filter: SampleFilter::Nearest,
    }
}

fn scene(image: &PreparedImageFixture, mode: u8, visibility: Option<golden_io::RevisionGuard>) -> CompiledPlan {
    let regions = match mode {
        0 => vec![
            region(
                1,
                UnitRect {
                    x: 0.0,
                    y: 0.0,
                    width: 0.5,
                    height: 0.5,
                },
                UnitRect {
                    x: 0.5,
                    y: 0.0,
                    width: 0.5,
                    height: 1.0,
                },
            ),
            region(
                2,
                UnitRect {
                    x: 0.5,
                    y: 0.5,
                    width: 0.5,
                    height: 0.5,
                },
                UnitRect::FULL,
            ),
        ],
        1 => vec![region(
            2,
            UnitRect {
                x: 0.0,
                y: 0.0,
                width: 0.25,
                height: 0.25,
            },
            UnitRect::FULL,
        )],
        _ => Vec::new(),
    };
    RenderPlan {
        external_textures: vec![ExternalTexture {
            id: TextureId::new(1).unwrap(),
            descriptor: image.asset.value().descriptor(),
            source: Some(ExternalSource::Image(image.asset.clone())),
        }],
        passes: vec![
            RenderPass {
                id: PassId::new(1).unwrap(),
                output: TextureId::new(2).unwrap(),
                descriptor: descriptor(2, 2),
                operation: RenderOperation::Clear(LinearColor::from_straight(0.0, 1.0, 0.0, 1.0).unwrap()),
            },
            RenderPass {
                id: PassId::new(2).unwrap(),
                output: TextureId::new(3).unwrap(),
                descriptor: descriptor(4, 4),
                operation: RenderOperation::Regions { regions, visibility },
            },
        ],
        outputs: vec![
            PlanOutput {
                id: OutputId::new(1).unwrap(),
                source: TextureId::new(3).unwrap(),
            },
            PlanOutput {
                id: OutputId::new(2).unwrap(),
                source: TextureId::new(2).unwrap(),
            },
        ],
    }
    .compile(&PlanLimits {
        max_passes: 8,
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
    device: DeviceGeneration,
    serial: u64,
}

impl Renderer {
    fn new(adapter: AdapterId, plan: &CompiledPlan) -> Self {
        let identity = Uuid::new_v4();
        let config = D3d11Config {
            adapter,
            inputs: InputLimits {
                max_sources: 0,
                max_storage_bytes: 0,
            },
            outputs: (1..=2)
                .map(|id| OutputBinding::Spout {
                    id: OutputId::new(id).unwrap(),
                    name: format!("GoldenRegions-{identity}-{id}"),
                    descriptor: descriptor(4, 4),
                })
                .collect(),
        };
        let device = DeviceGeneration::new(Uuid::new_v4(), 1);
        let mut backend = D3d11Backend::new(config, Default::default(), device);
        backend.initialize().unwrap();
        let resources = backend.prepare(plan, device).unwrap();
        assert!(backend.control.sources().is_empty());
        assert_eq!(backend.control.snapshot().uploaded_images, 1);
        Self {
            resources: Some(resources),
            backend,
            device,
            serial: 0,
        }
    }

    fn frame(&mut self, plan: &CompiledPlan) -> [Vec<[u8; 4]>; 2] {
        self.serial += 1;
        let time = TimeAuthority::new(SystemClock::new(), ShowPosition::from_nanos(0), PlaybackRate::NORMAL)
            .unwrap()
            .sample()
            .unwrap();
        self.backend
            .submit(
                self.resources.as_ref().unwrap(),
                plan,
                CompletionToken {
                    device: self.device,
                    serial: self.serial,
                },
                time,
            )
            .unwrap();
        let deadline = Instant::now() + WAIT;
        while self.backend.completion().unwrap().serial < self.serial {
            assert!(Instant::now() < deadline, "region frame completion timeout");
            std::thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(
            self.backend.control.snapshot().uploaded_images,
            1,
            "region layout must not upload immutable pixels per frame"
        );
        let outputs = self.backend.control.output_snapshots();
        for id in [OutputId::new(1).unwrap(), OutputId::new(2).unwrap()] {
            assert_eq!(
                outputs.iter().find(|output| output.id == id).unwrap().last_accepted,
                Some(CompletionToken {
                    device: self.device,
                    serial: self.serial
                })
            );
        }
        std::array::from_fn(|index| {
            crate::native::tests::pixels::readback(
                &self.backend.native.as_ref().unwrap().device,
                &self.resources.as_ref().unwrap().outputs[&OutputId::new(index as u64 + 1).unwrap()],
            )
        })
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
            "region pixel {actual:?} differs from {expected:?}"
        );
    }
    eprintln!("region output pixels {actual:?}, expected {expected:?}, tolerance2/255");
}

#[test]
#[ignore = "requires explicitly available Windows hardware D3D11"]
fn hardware_regions_crop_without_stretch_clear_removed_views_and_preserve_an_independent_output() {
    let (done, finished) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        if finished.recv_timeout(Duration::from_secs(60)).is_err() {
            eprintln!("region hardware watchdog expired before verified retirement");
            std::process::exit(1);
        }
    });
    let adapter = adapters()
        .unwrap()
        .into_iter()
        .find(|adapter| !adapter.software)
        .unwrap();
    eprintln!(
        "region hardware LUID {:08x}:{:08x}",
        adapter.id.high as u32, adapter.id.low
    );
    let pixels = [
        [255, 0, 0, 255],
        [0, 255, 0, 255],
        [0, 0, 255, 255],
        [255, 255, 0, 255],
        [0, 255, 255, 255],
        [255, 0, 255, 255],
        [255, 255, 255, 255],
        [0, 0, 0, 255],
    ]
    .concat();
    let image = prepared_image(descriptor(4, 2), &pixels);
    let retirement = image.retirement();
    let visibility = golden_io::RevisionSource::default();
    let initial = scene(&image, 0, Some(visibility.reader().guard_at(0)));
    let single = scene(&image, 1, None);
    let empty = scene(&image, 2, None);
    let mut renderer = Renderer::new(adapter.id, &initial);
    let first = renderer.frame(&initial);
    expect(
        &first[0],
        &[
            [0, 0, 255, 255],
            [255, 255, 0, 255],
            TRANSPARENT,
            TRANSPARENT,
            [255, 255, 255, 255],
            [0, 0, 0, 255],
            TRANSPARENT,
            TRANSPARENT,
            TRANSPARENT,
            TRANSPARENT,
            GREEN,
            GREEN,
            TRANSPARENT,
            TRANSPARENT,
            GREEN,
            GREEN,
        ],
    );
    expect(&first[1], &[GREEN; 16]);
    visibility.advance();
    assert!(
        initial.admission_is_current(),
        "presentation withdrawal must not reject healthy source execution"
    );
    // Do not allocate or install a replacement plan: withdrawal also applies to the active
    // generation when replacement preparation is delayed or under resource pressure.
    let withdrawn = renderer.frame(&initial);
    expect(&withdrawn[0], &[TRANSPARENT; 16]);
    expect(&withdrawn[1], &[GREEN; 16]);
    // Reuse the same admitted targets so clearing is proved against previous live pixels,
    // rather than depending on the initial contents of a new native allocation.
    let second = renderer.frame(&single);
    let mut one = [TRANSPARENT; 16];
    one[0] = GREEN;
    expect(&second[0], &one);
    expect(&second[1], &[GREEN; 16]);
    let third = renderer.frame(&empty);
    expect(&third[0], &[TRANSPARENT; 16]);
    expect(&third[1], &[GREEN; 16]);
    assert_eq!(renderer.backend.control.snapshot().accepted_outputs, 8);
    drop(renderer);
    drop(initial);
    drop(single);
    drop(empty);
    drop(image);
    assert!(retirement.wait(WAIT));
    done.send(()).unwrap();
}

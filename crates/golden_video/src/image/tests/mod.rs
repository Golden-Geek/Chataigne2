use super::*;
use crate::{AlphaMode, ColorEncoding, Extent2D, PixelFormat};
use golden_assets::{
    AssetBinding, AssetDecoder, AssetId, AssetInput, AssetLocator, AssetPreparer, AssetRecord, AssetRetirement,
    AssetScope, PreparationFailure, PreparationLimits, PreparedAsset,
};
use std::io::Read;
use std::num::NonZeroUsize;
use std::time::{Duration, Instant};

pub(crate) struct PreparedImageFixture {
    pub asset: PreparedAsset<ImageData>,
    pub binding: AssetBinding,
    _scope: AssetScope,
    _directory: tempfile::TempDir,
    preparer: AssetPreparer<RawDecoder>,
}

impl PreparedImageFixture {
    pub fn retirement(&self) -> AssetRetirement {
        self.preparer.retirement()
    }
}

struct RawDecoder(TextureDescriptor);
impl AssetDecoder for RawDecoder {
    type Output = ImageData;
    fn decode(&mut self, input: &mut AssetInput, maximum_bytes: usize) -> Result<ImageData, PreparationFailure> {
        let length = usize::try_from(input.length()).map_err(|_| PreparationFailure::Decode)?;
        if size_of::<ImageData>() + length > maximum_bytes {
            return Err(PreparationFailure::ResourcePressure);
        }
        let mut pixels = Vec::new();
        pixels
            .try_reserve_exact(length)
            .map_err(|_| PreparationFailure::ResourcePressure)?;
        pixels.resize(length, 0);
        input.read_exact(&mut pixels).map_err(PreparationFailure::from)?;
        ImageData::new(self.0, pixels).map_err(|_| PreparationFailure::Decode)
    }
    fn retained_bytes(&self, image: &ImageData) -> usize {
        image.retained_bytes()
    }
}

pub(crate) fn prepared_image(descriptor: TextureDescriptor, pixels: &[u8]) -> PreparedImageFixture {
    prepared_image_with_id(AssetId::new(), descriptor, pixels)
}

pub(crate) fn prepared_image_with_id(
    id: AssetId,
    descriptor: TextureDescriptor,
    pixels: &[u8],
) -> PreparedImageFixture {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(directory.path().join("pixels.bin"), pixels).unwrap();
    let scope = AssetScope::new(directory.path()).unwrap();
    let binding = AssetBinding::new(AssetRecord {
        id,
        locator: AssetLocator::ProjectRelative {
            path: "pixels.bin".to_string().try_into().unwrap(),
        },
    });
    let mut preparer = AssetPreparer::new(
        RawDecoder(descriptor),
        PreparationLimits {
            max_source_bytes: 4096,
            max_decode_bytes: NonZeroUsize::new(4096).unwrap(),
            max_prepared_bytes: NonZeroUsize::new(65536).unwrap(),
            ..Default::default()
        },
    )
    .unwrap();
    preparer.submit(scope.capture(&binding)).unwrap();
    let mut outcomes = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        preparer.drain(&mut outcomes, NonZeroUsize::MIN);
        if let Some(outcome) = outcomes.pop() {
            return PreparedImageFixture {
                asset: outcome.result.unwrap(),
                binding,
                _scope: scope,
                _directory: directory,
                preparer,
            };
        }
        assert!(
            Instant::now() < deadline,
            "immutable image fixture preparation timed out"
        );
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn descriptor(width: u32, height: u32) -> TextureDescriptor {
    TextureDescriptor {
        extent: Extent2D::new(width, height).unwrap(),
        format: PixelFormat::Rgba8Unorm,
        color_encoding: ColorEncoding::Srgb,
        alpha_mode: AlphaMode::Straight,
    }
}

#[test]
fn compiled_image_sources_validate_metadata_charge_leases_and_fence_relinked_assets() {
    use crate::plan::{ExternalSource, ExternalTexture, PlanLimits, PlanOutput, RenderPlan};
    use crate::{OutputId, TextureId};
    let image = prepared_image(descriptor(1, 1), &[255, 0, 0, 128]);
    let retirement = image.retirement();
    let plan = RenderPlan {
        external_textures: vec![ExternalTexture {
            id: TextureId::new(1).unwrap(),
            descriptor: image.asset.value().descriptor(),
            source: Some(ExternalSource::Image(image.asset.clone())),
        }],
        outputs: vec![PlanOutput {
            id: OutputId::new(1).unwrap(),
            source: TextureId::new(1).unwrap(),
        }],
        ..Default::default()
    };
    let limits = PlanLimits {
        max_passes: 4,
        max_external_textures: 4,
        max_outputs: 4,
        max_dependencies: 8,
        max_texture_dimension: 1024,
        max_texture_bytes: 65536,
    };
    assert!(plan.retained_bytes().unwrap() >= image.asset.retained_bytes());
    let compiled = plan.clone().compile(&limits).unwrap();
    assert_eq!(compiled.footprint().external_texture_bytes, 4);
    assert!(compiled.admission_is_current());
    let mut invalid = plan.clone();
    invalid.external_textures[0].descriptor.extent = Extent2D::new(2, 1).unwrap();
    assert!(matches!(
        invalid.compile(&limits),
        Err(crate::plan::PlanError::InvalidSourceBinding(_))
    ));
    image.binding.invalidate();
    assert!(!compiled.admission_is_current());
    assert!(matches!(
        plan.clone().compile(&limits),
        Err(crate::plan::PlanError::InvalidSourceBinding(_))
    ));
    drop(compiled);
    drop(plan);
    drop(image);
    assert!(retirement.wait(Duration::from_secs(5)));
}

#[test]
fn immutable_image_validates_tight_storage_and_does_not_retain_spare_capacity() {
    let descriptor = descriptor(2, 1);
    let mut pixels = Vec::with_capacity(1024);
    pixels.extend_from_slice(&[255, 0, 0, 128, 0, 0, 255, 255]);
    let image = ImageData::new(descriptor, pixels).unwrap();
    assert_eq!(image.descriptor(), descriptor);
    assert_eq!(image.pixels(), &[255, 0, 0, 128, 0, 0, 255, 255]);
    assert_eq!(image.retained_bytes(), size_of::<ImageData>() + 8);
    assert!(matches!(
        ImageData::new(descriptor, vec![0; 7]),
        Err(ImageError::Layout)
    ));
    assert!(matches!(
        ImageData::new(descriptor, vec![0; 9]),
        Err(ImageError::Layout)
    ));
    assert!(matches!(
        ImageData::new(self::descriptor(16_385, 1), Vec::new()),
        Err(ImageError::Dimension)
    ));
}

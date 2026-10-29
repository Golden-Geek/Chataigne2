use uuid::Uuid;

use crate::native::backend::D3d11Backend;
use crate::native::pipeline::CONSTANT_BYTES;
use crate::native::{AdapterId, D3d11Config, D3d11Control, InputLimits, NativeFailure, OutputBinding};
use crate::plan::SpoutSource;
use crate::plan::{ExternalTexture, PlanLimits, PlanOutput, RenderOperation, RenderPass, RenderPlan};
use crate::runtime::DeviceGeneration;
use crate::runtime::backend::{Backend, BackendError};
use crate::{
    AlphaMode, ColorEncoding, Extent2D, LinearColor, OutputId, PassId, PixelFormat, TextureDescriptor, TextureId,
};

fn descriptor() -> TextureDescriptor {
    TextureDescriptor {
        extent: Extent2D::new(1, 1).unwrap(),
        format: PixelFormat::Rgba8Unorm,
        color_encoding: ColorEncoding::Linear,
        alpha_mode: AlphaMode::Premultiplied,
    }
}

fn limits() -> PlanLimits {
    PlanLimits {
        max_passes: 8,
        max_external_textures: 8,
        max_outputs: 8,
        max_dependencies: 32,
        max_texture_dimension: 1024,
        max_texture_bytes: 1024,
    }
}

fn backend() -> (D3d11Backend, D3d11Control) {
    configured_backend(InputLimits::default())
}

fn configured_backend(inputs: InputLimits) -> (D3d11Backend, D3d11Control) {
    let control = D3d11Control::default();
    let backend = D3d11Backend::new(
        D3d11Config {
            adapter: AdapterId { low: 0, high: 0 },
            inputs,
            outputs: vec![OutputBinding::Spout {
                id: OutputId::new(1).unwrap(),
                name: "uninitialized-test".into(),
                descriptor: descriptor(),
            }],
        },
        control.clone(),
        DeviceGeneration::new(Uuid::new_v4(), 1),
    );
    (backend, control)
}

fn clear_plan(output: u64) -> RenderPlan {
    let texture = TextureId::new(1).unwrap();
    RenderPlan {
        external_textures: vec![],
        passes: vec![RenderPass {
            id: PassId::new(1).unwrap(),
            output: texture,
            descriptor: descriptor(),
            operation: RenderOperation::Clear(LinearColor::TRANSPARENT),
        }],
        outputs: vec![PlanOutput {
            id: OutputId::new(output).unwrap(),
            source: texture,
        }],
    }
}

#[test]
fn native_resource_charges_include_persistent_sdk_storage_and_generation_conversion() {
    let (backend, control) = backend();
    // One four-byte SDK share plus the pipeline's declared uniform storage; no GPU is created.
    assert_eq!(backend.base_resource_bytes().unwrap(), CONSTANT_BYTES + 4);
    assert_eq!(
        backend
            .resource_bytes(&clear_plan(1).compile(&limits()).unwrap())
            .unwrap(),
        8
    );
    assert!(!control.snapshot().initialized);
    assert!(control.snapshot().adapter.is_none());
}

#[test]
fn unsupported_inputs_and_unbound_outputs_are_rejected_without_silent_fallback() {
    let (backend, control) = backend();
    assert_eq!(
        backend.resource_bytes(&clear_plan(2).compile(&limits()).unwrap()),
        Err(BackendError::UnsupportedPlan)
    );
    assert_eq!(control.snapshot().failure, Some(NativeFailure::UnboundOutput));
    let external = RenderPlan {
        external_textures: vec![ExternalTexture {
            id: TextureId::new(1).unwrap(),
            descriptor: descriptor(),
            source: None,
        }],
        passes: vec![],
        outputs: vec![PlanOutput {
            id: OutputId::new(1).unwrap(),
            source: TextureId::new(1).unwrap(),
        }],
    }
    .compile(&limits())
    .unwrap();
    assert_eq!(backend.resource_bytes(&external), Err(BackendError::UnsupportedPlan));
    assert_eq!(
        control.snapshot().failure,
        Some(NativeFailure::UnsupportedExternalInput)
    );
    assert!(!control.snapshot().initialized);
}

#[test]
fn input_reservations_cover_maximum_pools_while_plans_charge_actual_extents() {
    let (backend, control) = configured_backend(InputLimits {
        max_sources: 1,
        max_storage_bytes: 5 * 4 * 4 * 4,
    });
    let compile = |width| {
        RenderPlan {
            external_textures: vec![ExternalTexture {
                id: TextureId::new(1).unwrap(),
                source: Some(
                    SpoutSource {
                        sender: "uninitialized-source".into(),
                        descriptor: TextureDescriptor {
                            extent: Extent2D::new(4, 4).unwrap(),
                            ..descriptor()
                        },
                        poll_interval: std::time::Duration::from_millis(16),
                    }
                    .into(),
                ),
                descriptor: TextureDescriptor {
                    extent: Extent2D::new(width, 2).unwrap(),
                    ..descriptor()
                },
            }],
            passes: vec![],
            outputs: vec![PlanOutput {
                id: OutputId::new(1).unwrap(),
                source: TextureId::new(1).unwrap(),
            }],
        }
        .compile(&limits())
    };
    // Uniform/sender storage plus five admitted4x4 image stores.
    assert_eq!(
        backend.base_resource_bytes().unwrap(),
        CONSTANT_BYTES + 4 + 5 * 4 * 4 * 4
    );
    // Actual2x2 generation input plus its1x1 output conversion.
    assert_eq!(backend.resource_bytes(&compile(2).unwrap()).unwrap(), 16 + 4);
    assert_eq!(backend.resource_bytes(&compile(3).unwrap()).unwrap(), 24 + 4);
    assert!(compile(5).is_err());
    assert!(!control.snapshot().initialized);
    assert!(
        control.sources().is_empty(),
        "charge validation must not start source workers"
    );
}

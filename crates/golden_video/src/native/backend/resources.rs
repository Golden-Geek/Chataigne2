use std::collections::HashMap;
use std::rc::{Rc, Weak};

use windows::Win32::Graphics::Direct3D11::ID3D11Device;
use windows::core::Result;

use crate::plan::{CompiledPlan, ExternalSource, RenderOperation};
use crate::{OutputId, PassId, TextureDescriptor, TextureId};

use super::super::texture::Texture;
use super::{D3d11Resources, NativeState};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Role {
    Image,
    External(TextureId),
    Pass(TextureId),
    Scratch(PassId),
    Output(OutputId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct Key {
    role: Role,
    descriptor: TextureDescriptor,
    image: Option<usize>,
}

/// Worker/device-local index. Only resource generations own textures; stale index entries
/// cannot keep GPU allocations or CPU asset leases alive beyond completion-based retirement.
#[derive(Default)]
pub(super) struct TextureCache(HashMap<Key, Weak<Texture>>);

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct PreparationCounts {
    pub created: u64,
    pub reused: u64,
    pub uploaded_images: u64,
    pub uploaded_bytes: u64,
}

impl TextureCache {
    pub fn prune(&mut self) {
        self.0.retain(|_, value| value.strong_count() != 0);
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    fn get(
        &mut self,
        key: Key,
        counts: &mut PreparationCounts,
        create: impl FnOnce() -> Result<Texture>,
    ) -> Result<Rc<Texture>> {
        if let Some(texture) = self.0.get(&key).and_then(Weak::upgrade) {
            counts.reused += 1;
            return Ok(texture);
        }
        let texture = Rc::new(create()?);
        counts.created += 1;
        self.0.insert(key, Rc::downgrade(&texture));
        Ok(texture)
    }

    fn mutable(
        &mut self,
        role: Role,
        descriptor: TextureDescriptor,
        device: &ID3D11Device,
        counts: &mut PreparationCounts,
    ) -> Result<Rc<Texture>> {
        self.get(
            Key {
                role,
                descriptor,
                image: None,
            },
            counts,
            || Texture::new(device, descriptor),
        )
    }
}

pub(super) fn prepare(
    native: &mut NativeState,
    plan: &CompiledPlan,
    generation: crate::runtime::GenerationInfo,
    counts: &mut PreparationCounts,
) -> Result<D3d11Resources> {
    native.textures.prune();
    let mut resources = D3d11Resources {
        generation,
        textures: Default::default(),
        scratch: Default::default(),
        outputs: Default::default(),
        input_sequences: Default::default(),
        _demands: None,
    };
    let cache = &mut native.textures;
    let device = &native.device.device;
    for external in plan.external_textures() {
        let role = Role::External(external.id);
        let texture = match &external.source {
            Some(ExternalSource::Image(image)) => {
                // The texture retains this exact preparation lease. Its payload address cannot
                // be recycled while an upgradeable cache entry exists, even across project edits.
                let key = Key {
                    role: Role::Image,
                    descriptor: external.descriptor,
                    image: Some(std::ptr::from_ref(image.value()) as usize),
                };
                let before = counts.created;
                let texture = cache.get(key, counts, || Texture::from_asset(device, image))?;
                if counts.created != before {
                    counts.uploaded_images += 1;
                    counts.uploaded_bytes += image.value().pixels().len() as u64;
                }
                texture
            }
            _ => {
                // Every installed generation revalidates its source contents before drawing.
                // Sharing storage never carries a prior generation's sequence/availability hint.
                resources
                    .input_sequences
                    .insert(external.id, std::cell::Cell::new(None));
                cache.mutable(role, external.descriptor, device, counts)?
            }
        };
        resources.textures.insert(external.id, texture);
    }
    for pass in plan.passes() {
        resources.textures.insert(
            pass.output,
            cache.mutable(Role::Pass(pass.output), pass.descriptor, device, counts)?,
        );
        if matches!(&pass.operation, RenderOperation::Composite { layers } if layers.len() > 1) {
            resources.scratch.insert(
                pass.id,
                cache.mutable(Role::Scratch(pass.id), pass.descriptor, device, counts)?,
            );
        }
    }
    for output in plan.outputs() {
        let descriptor = native.outputs.descriptor(output.id).expect("validated output binding");
        resources.outputs.insert(
            output.id,
            cache.mutable(Role::Output(output.id), descriptor, device, counts)?,
        );
    }
    resources._demands = Some(native.inputs.demands(plan));
    Ok(resources)
}

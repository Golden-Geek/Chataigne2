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
pub(super) struct TextureCache {
    entries: HashMap<Key, Weak<Texture>>,
    owner: crate::native::allocations::TextureAllocations,
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct PreparationCounts {
    pub created: u64,
    pub reused: u64,
    pub uploaded_images: u64,
    pub uploaded_bytes: u64,
}

impl TextureCache {
    pub fn new(owner: crate::native::allocations::TextureAllocations) -> Self {
        Self {
            entries: HashMap::new(),
            owner,
        }
    }

    pub fn prune(&mut self) {
        self.entries.retain(|_, value| value.strong_count() != 0);
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    fn get(
        &mut self,
        key: Key,
        counts: &mut PreparationCounts,
        create: impl FnOnce() -> Result<Texture>,
    ) -> Result<Rc<Texture>> {
        if let Some(texture) = self.entries.get(&key).and_then(Weak::upgrade) {
            counts.reused += 1;
            return Ok(texture);
        }
        let mut texture = create()?;
        texture.track(&self.owner)?;
        let texture = Rc::new(texture);
        counts.created += 1;
        self.entries.insert(key, Rc::downgrade(&texture));
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
            || match role {
                Role::External(_) => Texture::new_sampled(device, descriptor),
                _ => Texture::new(device, descriptor),
            },
        )
    }
}

pub(super) fn prepare(
    native: &mut NativeState,
    plan: &CompiledPlan,
    generation: crate::runtime::GenerationInfo,
    counts: &mut PreparationCounts,
    notify: Option<golden_io::WorkerNotifier>,
) -> Result<D3d11Resources> {
    native.textures.prune();
    let mut resources = D3d11Resources {
        presentation: std::cell::RefCell::new(super::presentation::Presentation::new(plan, notify)),
        content_time: std::cell::Cell::new(None),
        generation,
        textures: Default::default(),
        scratch: Default::default(),
        outputs: Default::default(),
        presented_regions: Default::default(),
        input_sequences: Default::default(),
        canvas_images: Default::default(),
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
        for image in pass.operation.canvas_images() {
            let identity = std::sync::Arc::as_ptr(image) as usize;
            if resources.canvas_images.contains_key(&identity) {
                continue;
            }
            let key = Key {
                role: Role::Image,
                descriptor: image.descriptor(),
                image: Some(identity),
            };
            let before = counts.created;
            let texture = cache.get(key, counts, || Texture::from_canvas_image(device, image))?;
            if counts.created != before {
                counts.uploaded_images += 1;
                counts.uploaded_bytes += image.pixels().len() as u64;
            }
            resources.canvas_images.insert(identity, texture);
        }
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
        // Resolve output-specific placements during preparation, never per-frame traversal.
        let mut needed = std::collections::HashSet::from([output.source]);
        let mut placements = Vec::new();
        for pass in plan.passes().iter().rev() {
            if !needed.contains(&pass.output) {
                continue;
            }
            needed.extend(pass.operation.sources());
            if let RenderOperation::Regions { regions, .. } = &pass.operation {
                placements.extend(regions.iter().filter_map(|region| region.placement.clone()));
            }
        }
        resources.presented_regions.insert(output.id, placements);
        if !native.outputs.needs_conversion_texture(output.id) {
            continue;
        }
        let descriptor = native.outputs.descriptor(output.id).expect("validated output binding");
        resources.outputs.insert(
            output.id,
            cache.mutable(Role::Output(output.id), descriptor, device, counts)?,
        );
    }
    resources._demands = Some(native.inputs.demands(plan));
    Ok(resources)
}

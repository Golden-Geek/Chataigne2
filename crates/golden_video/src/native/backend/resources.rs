use std::collections::HashMap;
use std::rc::{Rc, Weak};

use windows::Win32::Graphics::Direct3D11::ID3D11Device;
use windows::core::Result;

use crate::plan::{CompiledPlan, ExternalSource, RenderOperation, RenderPass};
use crate::runtime::ProjectEpoch;
use crate::shader::FragmentProgram;
use crate::{OutputId, PassId, TextureDescriptor, TextureId};

use super::super::pipeline::{ShaderClock, ShaderDraw, ShaderPass};
use super::super::texture::Texture;
use super::history::History;
use super::{D3d11Resources, NativeState};

/// Share bytecode resources per device and retain a logical pass clock across plan revisions.
#[derive(Default)]
pub(super) struct ShaderCache {
    programs: HashMap<usize, Weak<ShaderDraw>>,
    clocks: HashMap<(ProjectEpoch, PassId, uuid::Uuid), Weak<ShaderClock>>,
}

impl ShaderCache {
    fn prune(&mut self) {
        self.programs.retain(|_, value| value.strong_count() != 0);
        self.clocks.retain(|_, value| value.strong_count() != 0);
    }

    fn pass(
        &mut self,
        project: ProjectEpoch,
        pass: PassId,
        program: &FragmentProgram,
        device: &ID3D11Device,
    ) -> Result<Rc<ShaderPass>> {
        let program_key = program.bytecode().as_ptr() as usize;
        let draw = match self.programs.get(&program_key).and_then(Weak::upgrade) {
            Some(draw) => draw,
            None => {
                let draw = Rc::new(ShaderDraw::new(device, program)?);
                self.programs.insert(program_key, Rc::downgrade(&draw));
                draw
            }
        };
        let clock_key = (project, pass, program.effect());
        let clock = self.clocks.get(&clock_key).and_then(Weak::upgrade).unwrap_or_else(|| {
            let clock = Rc::new(ShaderClock::default());
            self.clocks.insert(clock_key, Rc::downgrade(&clock));
            clock
        });
        Ok(Rc::new(ShaderPass::new(draw, clock, program)))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Role {
    Image,
    External(TextureId),
    Pass(crate::runtime::ProjectEpoch, TextureId),
    History(HistoryKey, bool),
    Scratch,
    Output(OutputId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct HistoryKey {
    project: ProjectEpoch,
    output: TextureId,
    clock: PassId,
    program: usize,
    descriptor: TextureDescriptor,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct Key {
    role: Role,
    descriptor: TextureDescriptor,
    image: Option<usize>,
    stream: Option<uuid::Uuid>,
}

/// Worker/device-local index. Only resource generations own textures; stale index entries
/// cannot keep GPU allocations or CPU asset leases alive beyond completion-based retirement.
pub(super) struct TextureCache {
    entries: HashMap<Key, Weak<Texture>>,
    history: HashMap<HistoryKey, Weak<History>>,
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
            history: HashMap::new(),
            owner,
        }
    }

    pub fn prune(&mut self) {
        self.entries.retain(|_, value| value.strong_count() != 0);
        self.history.retain(|_, value| value.strong_count() != 0);
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
                stream: None,
            },
            counts,
            || match role {
                Role::External(_) => Texture::new_sampled(device, descriptor),
                _ => Texture::new(device, descriptor),
            },
        )
    }

    fn history(
        &mut self,
        key: HistoryKey,
        program: &FragmentProgram,
        device: &ID3D11Device,
        counts: &mut PreparationCounts,
    ) -> Result<Rc<History>> {
        if let Some(entry) = self.history.get(&key).and_then(Weak::upgrade) {
            counts.reused += 2;
            return Ok(entry);
        }
        let current = self.mutable(Role::History(key, false), key.descriptor, device, counts)?;
        let previous = self.mutable(Role::History(key, true), key.descriptor, device, counts)?;
        let entry = Rc::new(History::new(current, previous, program.clone()));
        self.history.insert(key, Rc::downgrade(&entry));
        Ok(entry)
    }
}

pub(super) fn composite_scratch(pass: &RenderPass) -> Option<TextureDescriptor> {
    matches!(&pass.operation, RenderOperation::Composite { layers } if layers.len() > 1).then_some(pass.descriptor)
}

pub(super) fn prepare(
    native: &mut NativeState,
    plan: &CompiledPlan,
    generation: crate::runtime::GenerationInfo,
    counts: &mut PreparationCounts,
    notify: Option<golden_io::WorkerNotifier>,
    recorder: Option<&crate::native::timing::Recorder>,
) -> Result<D3d11Resources> {
    let mut source_routes = Vec::new();
    native.textures.prune();
    native.shaders.prune();
    let mut resources = D3d11Resources {
        render_cache: std::cell::RefCell::new(super::cache::Cache::new(plan)),
        presentation: std::cell::RefCell::new(super::presentation::Presentation::new(plan, notify)),
        content_time: std::cell::Cell::new(None),
        generation,
        textures: Default::default(),
        scratch: Default::default(),
        shaders: Default::default(),
        history: Default::default(),
        outputs: Default::default(),
        conversion_slots: (0..plan.outputs().len()).collect(),
        presented_regions: Default::default(),
        input_sequences: Default::default(),
        gpu_imports: Default::default(),
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
                    stream: None,
                };
                let before = counts.created;
                let texture = cache.get(key, counts, || Texture::from_asset(device, image))?;
                if counts.created != before {
                    counts.uploaded_images += 1;
                    counts.uploaded_bytes += image.value().pixels().len() as u64;
                }
                texture
            }
            Some(ExternalSource::Frames(stream)) => cache.get(
                Key {
                    role,
                    descriptor: external.descriptor,
                    image: None,
                    stream: Some(stream.id()),
                },
                counts,
                || Texture::new_sampled(device, external.descriptor),
            )?,
            Some(ExternalSource::GpuFrames(stream)) => cache.get(
                Key {
                    role,
                    descriptor: external.descriptor,
                    image: None,
                    stream: Some(stream.id()),
                },
                counts,
                || Texture::new_sampled(device, external.descriptor),
            )?,
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
    for (index, pass) in plan.passes().iter().enumerate() {
        for image in pass.operation.canvas_images() {
            let identity = std::sync::Arc::as_ptr(image) as usize;
            if resources.canvas_images.contains_key(&identity) {
                continue;
            }
            let key = Key {
                role: Role::Image,
                descriptor: image.descriptor(),
                image: Some(identity),
                stream: None,
            };
            let before = counts.created;
            let texture = cache.get(key, counts, || Texture::from_canvas_image(device, image))?;
            if counts.created != before {
                counts.uploaded_images += 1;
                counts.uploaded_bytes += image.pixels().len() as u64;
            }
            resources.canvas_images.insert(identity, texture);
        }
        let target = if plan.history_targets.contains(&pass.output) {
            let RenderOperation::Shader { program, clock, .. } = &pass.operation else {
                unreachable!("compiled history target is owned by a shader");
            };
            let history = cache.history(
                HistoryKey {
                    project: generation.identity.project,
                    output: pass.output,
                    clock: *clock,
                    program: program.bytecode().as_ptr() as usize,
                    descriptor: pass.descriptor,
                },
                program,
                device,
                counts,
            )?;
            let target = history.current.clone();
            resources.history.insert(pass.output, (index, history));
            target
        } else {
            cache.mutable(
                Role::Pass(generation.identity.project, pass.output),
                pass.descriptor,
                device,
                counts,
            )?
        };
        resources.textures.insert(pass.output, target);
        if let Some(descriptor) = composite_scratch(pass) {
            // Scratch is assigned and consumed within one ordered composite pass. It is
            // never a graph output, so compatible passes can share its device-local storage.
            resources
                .scratch
                .insert(pass.id, cache.mutable(Role::Scratch, descriptor, device, counts)?);
        }
        if let RenderOperation::Shader { program, clock, .. } = &pass.operation {
            let shader = native
                .shaders
                .pass(generation.identity.project, *clock, program, device)?;
            resources.shaders.insert(pass.id, shader);
        }
    }
    let mut conversions = HashMap::new();
    for (index, output) in plan.outputs().iter().enumerate() {
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
        if recorder.is_some() {
            let sources = plan
                .external_textures()
                .iter()
                .filter(|source| needed.contains(&source.id));
            let count = sources.clone().count();
            let route = |texture| crate::native::timing::OutputSourceTrace {
                generation,
                output: output.id,
                texture,
                source_count: count,
            };
            if count == 0 {
                source_routes.push(route(None));
            } else {
                source_routes.extend(sources.map(|source| route(Some(source.id))));
            }
        }
        if !native.outputs.needs_conversion_texture(output.id) {
            continue;
        }
        let descriptor = native.outputs.descriptor(output.id).expect("validated output binding");
        // Exact source and format share conversion pixels. Each SDK sender still owns
        // its own admitted endpoint and shared texture.
        let (slot, owner) = *conversions
            .entry((output.source, descriptor))
            .or_insert((index, output.id));
        resources.conversion_slots[index] = slot;
        resources.outputs.insert(
            output.id,
            cache.mutable(Role::Output(owner), descriptor, device, counts)?,
        );
    }
    resources._demands = Some(native.inputs.demands(plan));
    if let Some(recorder) = recorder {
        for route in source_routes {
            recorder.push(crate::native::timing::DiagnosticEvent::OutputSource(route));
        }
    }
    Ok(resources)
}

use std::collections::{BTreeMap, BTreeSet};

use crate::{PassId, TextureDescriptor, TextureId};

use super::{CompiledPlan, GraphLimit, PlanError, PlanLimits, RenderOperation, RenderPlan, TextureFootprint};

#[derive(Clone, Copy)]
enum Writer {
    External,
    Pass(usize),
}

struct Resource {
    writer: Writer,
    bytes: u64,
}

pub(super) fn compile(mut plan: RenderPlan, limits: &PlanLimits) -> Result<CompiledPlan, PlanError> {
    check_submitted_work(&plan, limits)?;

    let mut pass_indices = BTreeMap::new();
    for (index, pass) in plan.passes.iter().enumerate() {
        if pass_indices.insert(pass.id, index).is_some() {
            return Err(PlanError::DuplicatePass(pass.id));
        }
    }

    let mut resources = BTreeMap::new();
    for texture in &plan.external_textures {
        let bytes = validate_descriptor(texture.id, &texture.descriptor, limits)?;
        insert_resource(&mut resources, texture.id, Writer::External, bytes)?;
    }
    for (index, pass) in plan.passes.iter().enumerate() {
        let bytes = validate_descriptor(pass.output, &pass.descriptor, limits)?;
        if !pass.descriptor.is_compositing_target() {
            return Err(PlanError::InvalidTargetContract {
                pass: pass.id,
                output: pass.output,
            });
        }
        insert_resource(&mut resources, pass.output, Writer::Pass(index), bytes)?;
    }

    let mut output_ids = BTreeSet::new();
    for output in &plan.outputs {
        if !output_ids.insert(output.id) {
            return Err(PlanError::DuplicateOutput(output.id));
        }
        if !resources.contains_key(&output.source) {
            return Err(PlanError::MissingOutputSource {
                output: output.id,
                source: output.source,
            });
        }
    }

    // Validate every branch before pruning. Kahn's algorithm is iterative even for very
    // deep composition chains; the ordered ready set gives stable pass-ID tie breaking.
    let ordered = topological_order(&plan, &pass_indices, &resources)?;
    let demanded = demanded_textures(&plan, &resources);
    let footprint = footprint(&demanded, &resources, limits.max_texture_bytes)?;
    let mut rank = vec![0; plan.passes.len()];
    for (position, index) in ordered.into_iter().enumerate() {
        rank[index] = position;
    }

    // Retain reuses the submitted vectors and drops inactive metadata during preparation,
    // rather than keeping hidden inactive passes/resources in the executable plan.
    plan.external_textures.retain(|texture| demanded.contains(&texture.id));
    plan.external_textures.sort_unstable_by_key(|texture| texture.id);
    plan.passes.retain(|pass| demanded.contains(&pass.output));
    plan.passes.sort_unstable_by_key(|pass| rank[pass_indices[&pass.id]]);
    for pass in &mut plan.passes {
        if let RenderOperation::Composite { layers } = &mut pass.operation {
            layers.shrink_to_fit();
        }
    }
    plan.outputs.sort_unstable_by_key(|output| output.id);
    Ok(CompiledPlan {
        external_textures: plan.external_textures.into_boxed_slice(),
        passes: plan.passes.into_boxed_slice(),
        outputs: plan.outputs.into_boxed_slice(),
        footprint,
    })
}

fn check_submitted_work(plan: &RenderPlan, limits: &PlanLimits) -> Result<(), PlanError> {
    for (kind, actual, limit) in [
        (GraphLimit::Passes, plan.passes.len(), limits.max_passes),
        (
            GraphLimit::ExternalTextures,
            plan.external_textures.len(),
            limits.max_external_textures,
        ),
        (GraphLimit::Outputs, plan.outputs.len(), limits.max_outputs),
    ] {
        check_count(kind, actual, limit)?;
    }
    let dependencies = plan.passes.iter().try_fold(0_usize, |count, pass| {
        count
            .checked_add(pass.operation.dependency_count())
            .ok_or(PlanError::DependencyCountOverflow)
    })?;
    check_count(GraphLimit::Dependencies, dependencies, limits.max_dependencies)
}

fn check_count(kind: GraphLimit, actual: usize, limit: usize) -> Result<(), PlanError> {
    if actual > limit {
        return Err(PlanError::GraphLimitExceeded { kind, actual, limit });
    }
    Ok(())
}

fn validate_descriptor(id: TextureId, descriptor: &TextureDescriptor, limits: &PlanLimits) -> Result<u64, PlanError> {
    let width = descriptor.extent.width();
    let height = descriptor.extent.height();
    if width > limits.max_texture_dimension || height > limits.max_texture_dimension {
        return Err(PlanError::TextureDimensionExceeded {
            texture: id,
            width,
            height,
            limit: limits.max_texture_dimension,
        });
    }
    descriptor
        .byte_len()
        .map_err(|source| PlanError::InvalidTexture { texture: id, source })
}

fn insert_resource(
    resources: &mut BTreeMap<TextureId, Resource>,
    id: TextureId,
    writer: Writer,
    bytes: u64,
) -> Result<(), PlanError> {
    if resources.insert(id, Resource { writer, bytes }).is_some() {
        return Err(PlanError::DuplicateTextureWriter(id));
    }
    Ok(())
}

fn topological_order(
    plan: &RenderPlan,
    pass_indices: &BTreeMap<PassId, usize>,
    resources: &BTreeMap<TextureId, Resource>,
) -> Result<Vec<usize>, PlanError> {
    let mut dependents = vec![Vec::new(); plan.passes.len()];
    let mut indegrees = vec![0_usize; plan.passes.len()];
    for (index, pass) in plan.passes.iter().enumerate() {
        for source in pass.operation.sources() {
            let resource = resources
                .get(&source)
                .ok_or(PlanError::MissingPassInput { pass: pass.id, source })?;
            if let Writer::Pass(producer) = resource.writer {
                dependents[producer].push(index);
                indegrees[index] = indegrees[index]
                    .checked_add(1)
                    .ok_or(PlanError::DependencyCountOverflow)?;
            }
        }
    }
    let mut ready = pass_indices
        .iter()
        .filter_map(|(&id, &index)| (indegrees[index] == 0).then_some(id))
        .collect::<BTreeSet<_>>();
    let mut ordered = Vec::with_capacity(plan.passes.len());
    while let Some(id) = ready.pop_first() {
        let index = pass_indices[&id];
        ordered.push(index);
        for &dependent in &dependents[index] {
            indegrees[dependent] -= 1;
            if indegrees[dependent] == 0 {
                ready.insert(plan.passes[dependent].id);
            }
        }
    }
    if ordered.len() != plan.passes.len() {
        let blocked_passes = pass_indices
            .iter()
            .filter_map(|(&id, &index)| (indegrees[index] != 0).then_some(id))
            .collect();
        return Err(PlanError::Cycle { blocked_passes });
    }
    Ok(ordered)
}

fn demanded_textures(plan: &RenderPlan, resources: &BTreeMap<TextureId, Resource>) -> BTreeSet<TextureId> {
    let mut demanded = BTreeSet::new();
    let mut pending = Vec::new();
    for output in &plan.outputs {
        if demanded.insert(output.source) {
            pending.push(output.source);
        }
    }
    while let Some(id) = pending.pop() {
        if let Writer::Pass(index) = resources[&id].writer {
            for source in plan.passes[index].operation.sources() {
                if demanded.insert(source) {
                    pending.push(source);
                }
            }
        }
    }
    demanded
}

fn footprint(
    demanded: &BTreeSet<TextureId>,
    resources: &BTreeMap<TextureId, Resource>,
    limit: u64,
) -> Result<TextureFootprint, PlanError> {
    let mut footprint = TextureFootprint::default();
    for id in demanded {
        let resource = &resources[id];
        let bytes = match resource.writer {
            Writer::External => &mut footprint.external_texture_bytes,
            Writer::Pass(_) => &mut footprint.target_texture_bytes,
        };
        *bytes = bytes
            .checked_add(resource.bytes)
            .ok_or(PlanError::TextureFootprintOverflow)?;
    }
    footprint.total_texture_bytes = footprint
        .external_texture_bytes
        .checked_add(footprint.target_texture_bytes)
        .ok_or(PlanError::TextureFootprintOverflow)?;
    if footprint.total_texture_bytes > limit {
        return Err(PlanError::TextureBudgetExceeded {
            actual: footprint.total_texture_bytes,
            limit,
        });
    }
    Ok(footprint)
}

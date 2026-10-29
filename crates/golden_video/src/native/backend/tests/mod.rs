#[cfg(feature = "qualification")]
pub(in crate::native) mod copy_tap;
mod images;
mod inputs;
mod regions;
mod startup;

fn generation(
    backend: &super::D3d11Backend,
    plan: &crate::plan::CompiledPlan,
    device: crate::runtime::DeviceGeneration,
) -> crate::runtime::GenerationInfo {
    use crate::runtime::backend::Backend;
    use crate::runtime::{GenerationId, GenerationInfo, PlanIdentity, PlanRevision, ProjectEpoch};
    GenerationInfo {
        generation: GenerationId::new(1),
        identity: PlanIdentity {
            project: ProjectEpoch::new(uuid::Uuid::nil(), 0),
            revision: PlanRevision::new(1),
        },
        device,
        // Invalid direct-backend cases have no admitted charge; Core rejects those earlier.
        bytes: backend.resource_bytes(plan).unwrap_or(0),
        output_count: plan.outputs().len(),
    }
}

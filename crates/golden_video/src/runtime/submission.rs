use std::sync::Arc;

use golden_io::RevisionGuard;

use crate::TextureId;
use crate::plan::{CompiledPlan, ExternalSource, PlanError, PlanLimits, RenderPlan};

/// Retained CPU preparation input. Source-only roots can warm receivers without inventing
/// an output extent; guards fence app-owned authoring state before preparation/installation.
#[derive(Clone, Debug)]
pub struct PlanSubmission {
    /// Logical render graph.
    pub plan: RenderPlan,
    /// External textures demanded independently of presentation outputs.
    pub source_demands: Vec<TextureId>,
    /// Immutable authoring fences. These never contain native resources.
    pub guards: Arc<[RevisionGuard]>,
    /// Optional app-neutral final-use observation, retained until resources actually retire.
    pub lifetime: Option<super::PlanLifetime>,
}

impl PlanSubmission {
    /// Ordinary presentation demand with no additional authoring fences.
    pub fn new(plan: RenderPlan) -> Self {
        Self {
            plan,
            source_demands: Vec::new(),
            guards: Arc::from([]),
            lifetime: None,
        }
    }

    pub(crate) fn current(&self) -> bool {
        self.guards.iter().all(RevisionGuard::is_current)
            && self
                .plan
                .external_textures
                .iter()
                .all(|texture| texture.source.as_ref().is_none_or(ExternalSource::current))
    }

    pub(crate) fn attach_image_guards(&mut self, maximum: usize) -> Option<()> {
        let count = self
            .plan
            .external_textures
            .iter()
            .try_fold(self.guards.len(), |count, texture| match &texture.source {
                Some(ExternalSource::Image(image)) => count.checked_add(image.guards().len()),
                _ => Some(count),
            })?;
        if count > maximum {
            return None;
        }
        if count != self.guards.len() {
            let mut guards = Vec::with_capacity(count);
            guards.extend_from_slice(&self.guards);
            for texture in &self.plan.external_textures {
                if let Some(ExternalSource::Image(image)) = &texture.source {
                    guards.extend_from_slice(image.guards());
                }
            }
            self.guards = Arc::from(guards);
        }
        Some(())
    }

    pub(crate) fn retained_bytes(&self) -> Option<usize> {
        self.plan
            .retained_bytes()?
            .checked_add(size_of::<Self>())?
            .checked_add(self.source_demands.capacity().checked_mul(size_of::<TextureId>())?)?
            .checked_add(self.guards.iter().try_fold(0_usize, |bytes, guard| {
                bytes.checked_add(size_of::<RevisionGuard>() + guard.retained_heap_bytes())
            })?)?
            .checked_add(2 * size_of::<usize>())?
            .checked_add(
                self.lifetime
                    .as_ref()
                    .map_or(0, super::PlanLifetime::retained_heap_bytes),
            )
    }

    pub(crate) fn compile(self, limits: &PlanLimits) -> Result<CompiledPlan, PlanError> {
        let mut compiled = self.plan.compile_with_source_demands(limits, &self.source_demands)?;
        compiled.guards = self.guards;
        compiled.lifetime = self.lifetime;
        Ok(compiled)
    }
}

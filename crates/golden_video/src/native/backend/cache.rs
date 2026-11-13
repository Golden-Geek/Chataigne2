//! Pass contents belong to actual target writes, not a submission clock or source request.
use crate::TextureId;
use crate::plan::{CompiledPlan, LayerSource, Pattern, RenderOperation};
use crate::runtime::CompletionToken;

pub(super) struct Cache {
    passes: Vec<Pass>,
}

struct Pass {
    sources: Vec<(TextureId, Option<CompletionToken>)>,
    written: Option<CompletionToken>,
    dynamic: bool,
}

impl Cache {
    pub(super) fn new(plan: &CompiledPlan) -> Self {
        let untracked: std::collections::HashSet<_> = plan
            .external_textures()
            .iter()
            .filter(|source| source.source.is_none())
            .map(|source| source.id)
            .collect();
        Self {
            passes: plan
                .passes()
                .iter()
                .map(|pass| {
                    let dynamic = match &pass.operation {
                        // Placement and visibility may change independently of a plan install.
                        RenderOperation::Regions { .. } => true,
                        RenderOperation::Composite { layers } => layers.iter().any(|layer| {
                            layer.opacity_envelope.is_some()
                                || matches!(layer.source, LayerSource::Pattern(Pattern::Canvas(_)))
                        }),
                        _ => false,
                    } || pass.operation.sources().any(|source| untracked.contains(&source));
                    Pass {
                        sources: pass.operation.sources().map(|source| (source, None)).collect(),
                        written: None,
                        dynamic,
                    }
                })
                .collect(),
        }
    }

    pub(super) fn needs_render(
        &self,
        index: usize,
        actual_output: Option<CompletionToken>,
        input: impl Fn(TextureId) -> Option<CompletionToken>,
    ) -> bool {
        let pass = &self.passes[index];
        pass.dynamic
            || pass.written.is_none()
            // Compatible generations can share an allocation; another writer invalidates us.
            || pass.written != actual_output
            || pass.sources.iter().any(|(source, sampled)| *sampled != input(*source))
    }

    pub(super) fn rendered(
        &mut self,
        index: usize,
        token: CompletionToken,
        input: impl Fn(TextureId) -> Option<CompletionToken>,
    ) {
        let pass = &mut self.passes[index];
        for (source, sampled) in &mut pass.sources {
            *sampled = input(*source);
        }
        pass.written = Some(token);
    }
}

#[cfg(test)]
mod tests;

//! Pass contents belong to actual target writes, not a submission clock or source request.
use crate::TextureId;
use crate::plan::{CompiledPlan, LayerSource, Pattern, RenderOperation};
use crate::runtime::CompletionToken;

pub(super) struct Cache {
    passes: Vec<Pass>,
    conversions: Vec<Pass>,
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
                        RenderOperation::Clear(_) | RenderOperation::Sample { .. } => false,
                    } || pass.operation.sources().any(|source| untracked.contains(&source));
                    Pass {
                        sources: pass.operation.sources().map(|source| (source, None)).collect(),
                        written: None,
                        dynamic,
                    }
                })
                .collect(),
            conversions: plan
                .outputs()
                .iter()
                .map(|output| Pass {
                    sources: vec![(output.source, None)],
                    written: None,
                    dynamic: untracked.contains(&output.source),
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
        self.passes[index].needs_render(actual_output, input)
    }

    pub(super) fn needs_conversion(
        &self,
        index: usize,
        actual_output: Option<CompletionToken>,
        input: impl Fn(TextureId) -> Option<CompletionToken>,
    ) -> bool {
        self.conversions[index].needs_render(actual_output, input)
    }

    pub(super) fn converted(
        &mut self,
        index: usize,
        token: CompletionToken,
        input: impl Fn(TextureId) -> Option<CompletionToken>,
    ) {
        self.conversions[index].rendered(token, input);
    }

    pub(super) fn rendered(
        &mut self,
        index: usize,
        token: CompletionToken,
        input: impl Fn(TextureId) -> Option<CompletionToken>,
    ) {
        self.passes[index].rendered(token, input);
    }
}

impl Pass {
    fn needs_render(
        &self,
        actual_output: Option<CompletionToken>,
        input: impl Fn(TextureId) -> Option<CompletionToken>,
    ) -> bool {
        self.dynamic
            || self.written.is_none()
            // Compatible generations can share an allocation; another writer invalidates us.
            || self.written != actual_output
            || self.sources.iter().any(|(source, sampled)| *sampled != input(*source))
    }

    fn rendered(&mut self, token: CompletionToken, input: impl Fn(TextureId) -> Option<CompletionToken>) {
        for (source, sampled) in &mut self.sources {
            *sampled = input(*source);
        }
        self.written = Some(token);
    }
}

#[cfg(test)]
mod tests;

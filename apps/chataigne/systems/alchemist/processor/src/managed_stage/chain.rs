use std::sync::Arc;

use chataigne_alchemist::{
    ChannelLayout, CompileCtx, ContextKey, DebugCaptureMode, EvaluationCtx, ManagedFilterValueMode, ManagedItemId,
    ManagedItemInstance, RuntimeOutput, SocketId,
};
use indexmap::IndexSet;

use crate::{ChannelFrame, RuntimeInputBinding};

use super::{ManagedStageError, ManagedStageRuntime, ManagedStageSpecializationCache};

pub struct ManagedStageChain {
    input_layout: Arc<ChannelLayout>,
    output_layout: Arc<ChannelLayout>,
    stages: Vec<ManagedStageRuntime>,
}

impl ManagedStageChain {
    pub fn compile(
        items: &[ManagedItemInstance],
        input_layout: Arc<ChannelLayout>,
        ctx: &CompileCtx<'_>,
        mode: ManagedFilterValueMode,
    ) -> Result<Self, ManagedStageError> {
        Self::compile_with_cache(items, input_layout, ctx, mode, None)
    }

    pub fn compile_with_cache(
        items: &[ManagedItemInstance],
        input_layout: Arc<ChannelLayout>,
        ctx: &CompileCtx<'_>,
        mode: ManagedFilterValueMode,
        mut cache: Option<&mut ManagedStageSpecializationCache>,
    ) -> Result<Self, ManagedStageError> {
        let mut output_layout = Arc::clone(&input_layout);
        let mut stages = Vec::with_capacity(items.len());
        for item in items.iter().filter(|item| item.enabled && item.anode.enabled) {
            if let Some(stage) =
                ManagedStageRuntime::compile_with_cache(item.clone(), &output_layout, ctx, mode, cache.as_deref_mut())?
            {
                output_layout = Arc::clone(stage.output_layout());
                stages.push(stage);
            }
        }
        Ok(Self {
            input_layout,
            output_layout,
            stages,
        })
    }

    #[must_use]
    pub fn input_layout(&self) -> &Arc<ChannelLayout> {
        &self.input_layout
    }

    #[must_use]
    pub fn output_layout(&self) -> &Arc<ChannelLayout> {
        &self.output_layout
    }

    #[must_use]
    pub fn needs_continuous_evaluation(&self) -> bool {
        self.stages.iter().any(ManagedStageRuntime::needs_continuous_evaluation)
    }

    pub fn update_runtime_input(
        &mut self,
        item: ManagedItemId,
        socket: &SocketId,
        binding: RuntimeInputBinding,
    ) -> Result<(), ManagedStageError> {
        self.stages
            .iter_mut()
            .find(|stage| stage.item_id() == item)
            .ok_or(ManagedStageError::MissingStage(item))?
            .update_runtime_input(socket, binding)
    }

    pub fn reset_memory(&mut self) {
        for stage in &mut self.stages {
            stage.reset_memory();
        }
    }

    pub fn suspend_context(&mut self, context_key: &ContextKey) {
        for stage in &mut self.stages {
            stage.suspend_context(context_key);
        }
    }

    pub fn retain_context_keys(&mut self, active: &IndexSet<ContextKey>) {
        for stage in &mut self.stages {
            stage.retain_context_keys(active);
        }
    }

    pub fn migrate_memory_from(&mut self, previous: Self) {
        for (stage, old_stage) in self.stages.iter_mut().zip(previous.stages) {
            if !stage.migrate_memory_from(old_stage) {
                break;
            }
        }
    }

    pub fn evaluate<'a>(
        &'a mut self,
        input: &'a ChannelFrame,
        ctx: &EvaluationCtx<'_>,
    ) -> Result<(&'a ChannelFrame, RuntimeOutput), ManagedStageError> {
        self.evaluate_with_capture(input, ctx, DebugCaptureMode::Off)
    }

    pub fn evaluate_with_capture<'a>(
        &'a mut self,
        input: &'a ChannelFrame,
        ctx: &EvaluationCtx<'_>,
        capture_mode: DebugCaptureMode,
    ) -> Result<(&'a ChannelFrame, RuntimeOutput), ManagedStageError> {
        self.evaluate_with_capture_for_context(input, ctx, capture_mode, &ContextKey::default_lane())
    }

    pub fn evaluate_with_capture_for_context<'a>(
        &'a mut self,
        input: &'a ChannelFrame,
        ctx: &EvaluationCtx<'_>,
        capture_mode: DebugCaptureMode,
        context_key: &ContextKey,
    ) -> Result<(&'a ChannelFrame, RuntimeOutput), ManagedStageError> {
        if !input.layout().has_same_structure(&self.input_layout) {
            return Err(ManagedStageError::InputLayoutChanged);
        }
        let mut output = RuntimeOutput::default();
        for index in 0..self.stages.len() {
            let (previous, remaining) = self.stages.split_at_mut(index);
            let current = previous.last().map_or(input, |stage| &stage.output_frame);
            let (_, stage_output) =
                remaining[0].evaluate_with_capture_for_context(current, ctx, capture_mode.clone(), context_key)?;
            output.intents.extend(stage_output.intents);
            output.diagnostics.extend(stage_output.diagnostics);
            output.debug_samples.extend(stage_output.debug_samples);
        }
        if !output.diagnostics.is_empty() {
            output.intents.clear();
            output.debug_samples.clear();
        }
        Ok((self.stages.last().map_or(input, |stage| &stage.output_frame), output))
    }
}

use crate::AudioError;

use super::{PlanarBuffer, RenderPlan, RenderProcessor};

#[derive(Debug)]
pub struct OfflineRenderer {
    processor: RenderProcessor,
}

impl OfflineRenderer {
    pub fn new(plan: RenderPlan) -> Result<Self, AudioError> {
        Ok(Self {
            processor: RenderProcessor::new(plan)?,
        })
    }

    #[must_use]
    pub fn processor(&self) -> &RenderProcessor {
        &self.processor
    }

    pub fn processor_mut(&mut self) -> &mut RenderProcessor {
        &mut self.processor
    }

    pub fn render(
        &mut self,
        physical_inputs: &PlanarBuffer,
        playback_inputs: &PlanarBuffer,
        frames: usize,
    ) -> Result<PlanarBuffer, AudioError> {
        let mut output = PlanarBuffer::new(self.processor.plan().physical_outputs.len(), frames)?;
        self.processor
            .render(physical_inputs, playback_inputs, &mut output, frames)?;
        Ok(output)
    }
}

use std::collections::{BTreeMap, HashMap};

use golden_condition::ConditionInputId;
use golden_processor::{ProcessorBatchReport, ProcessorRuntime, ProcessorRuntimeError};
use golden_statechart::{StatechartRuntime, StatechartStep};
use golden_values::Value;
use smol_str::SmolStr;
use thiserror::Error;

pub struct ChataigneControlRuntime {
    statechart: StatechartRuntime,
    processors: BTreeMap<SmolStr, ProcessorRuntime>,
}

impl ChataigneControlRuntime {
    pub fn new(statechart: StatechartRuntime) -> Self {
        Self {
            statechart,
            processors: BTreeMap::new(),
        }
    }

    pub fn register_processor(&mut self, name: SmolStr, processor: ProcessorRuntime) -> bool {
        self.processors.insert(name, processor).is_none()
    }

    pub fn step(
        &mut self,
        event: Option<&str>,
        guards: &HashMap<SmolStr, bool>,
        condition_inputs: &BTreeMap<SmolStr, Vec<BTreeMap<ConditionInputId, Value>>>,
    ) -> Result<CompositionStep, CompositionError> {
        let statechart = self.statechart.step(event, guards);
        let mut processors = Vec::with_capacity(statechart.processors.len());
        for invocation in &statechart.processors {
            let processor = self
                .processors
                .get_mut(&invocation.processor)
                .ok_or_else(|| CompositionError::MissingProcessor(invocation.processor.clone()))?;
            let inputs = condition_inputs
                .get(&invocation.processor)
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            processors.push((
                invocation.processor.clone(),
                processor.evaluate(inputs, golden_alchemist::EvaluationOptions::default())?,
            ));
        }
        Ok(CompositionStep { statechart, processors })
    }
}

pub struct CompositionStep {
    pub statechart: StatechartStep,
    pub processors: Vec<(SmolStr, ProcessorBatchReport)>,
}

#[derive(Debug, Error)]
pub enum CompositionError {
    #[error("statechart references an unregistered processor: {0}")]
    MissingProcessor(SmolStr),
    #[error(transparent)]
    Processor(#[from] ProcessorRuntimeError),
}

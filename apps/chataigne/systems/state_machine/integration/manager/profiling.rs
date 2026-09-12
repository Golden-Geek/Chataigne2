use std::sync::Arc;

use chataigne_alchemist::{AlchemistFormula, CompiledAlchemistFormula, RuntimeInputSnapshot};
use chataigne_state_machine::Processor;

use super::StateMachineManager;

pub(crate) struct RuntimeScaleFixture {
    pub processor: Processor,
    pub formula: AlchemistFormula,
    pub compiled: Arc<CompiledAlchemistFormula>,
    pub inputs: RuntimeInputSnapshot,
    pub managed: bool,
}

impl StateMachineManager {
    pub(crate) fn enable_runtime_scale_input_capture(&mut self) {
        self.runtime_cache.scale_captured_inputs.clear();
        self.runtime_cache.scale_input_capture_enabled = true;
    }

    pub(crate) fn runtime_scale_fixtures(&self) -> Vec<RuntimeScaleFixture> {
        eprintln!(
            "scale capture: enabled={} processors={} inputs={}",
            self.runtime_cache.scale_input_capture_enabled,
            self.runtime_cache.processors.len(),
            self.runtime_cache.scale_captured_inputs.len(),
        );
        let mut fixtures = self
            .runtime_cache
            .processors
            .values()
            .filter_map(|entry| {
                Some(RuntimeScaleFixture {
                    processor: entry.processor.clone(),
                    formula: entry.formula.clone(),
                    compiled: Arc::clone(entry.runtime.compiled.as_ref()?),
                    inputs: self
                        .runtime_cache
                        .scale_captured_inputs
                        .get(&entry.processor.id)
                        .cloned()?,
                    managed: entry.runtime.managed_formula.is_some(),
                })
            })
            .collect::<Vec<_>>();
        fixtures.sort_by(|left, right| left.processor.label.cmp(&right.processor.label));
        fixtures
    }
}

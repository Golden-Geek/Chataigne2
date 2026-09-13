//! Context axes required to evaluate and retain one processor Formula.

use chataigne_alchemist::{AxisSet, ContextAxisId, ContextKey, ContextValuePath, FormulaAnalysis};
use golden_values::Value as RuntimeValue;

use super::{ProcessorContextProvider, ProcessorId};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProcessorBindingAnalysis {
    pub property_axes: AxisSet,
    pub input_axes: AxisSet,
    pub output_axes: AxisSet,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProcessorExecutionStrategy {
    SingleStateless,
    MultiStateless,
    SingleStateful,
    MultiStatefulSparse,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcessorExecutionPlan {
    pub processor_id: ProcessorId,
    pub available_axes: AxisSet,
    pub required_eval_axes: AxisSet,
    pub required_memory_axes: AxisSet,
    pub strategy: ProcessorExecutionStrategy,
}

impl ProcessorExecutionPlan {
    #[must_use]
    pub fn analyze(
        processor_id: ProcessorId,
        formula: &FormulaAnalysis,
        bindings: &ProcessorBindingAnalysis,
        available_axes: AxisSet,
    ) -> Self {
        let mut required_eval_axes = AxisSet::new();
        extend_axes(&mut required_eval_axes, &bindings.property_axes);
        extend_axes(&mut required_eval_axes, &formula.explicit_context_axes);
        extend_axes(&mut required_eval_axes, &bindings.input_axes);
        extend_axes(&mut required_eval_axes, &bindings.output_axes);
        extend_axes(&mut required_eval_axes, &formula.effect_axes);

        let mut required_memory_axes = AxisSet::new();
        if formula.has_stateful_nodes {
            extend_axes(&mut required_memory_axes, &formula.state_axes);
            extend_axes(&mut required_memory_axes, &bindings.property_axes);
            extend_axes(&mut required_memory_axes, &bindings.input_axes);
        }
        if formula.has_input_gated_nodes {
            extend_axes(&mut required_memory_axes, &bindings.property_axes);
            extend_axes(&mut required_memory_axes, &formula.explicit_context_axes);
            extend_axes(&mut required_memory_axes, &bindings.input_axes);
            extend_axes(&mut required_memory_axes, &formula.effect_axes);
        }

        let strategy = match (formula.has_stateful_nodes, required_eval_axes.is_empty()) {
            (false, true) => ProcessorExecutionStrategy::SingleStateless,
            (false, false) => ProcessorExecutionStrategy::MultiStateless,
            (true, true) => ProcessorExecutionStrategy::SingleStateful,
            (true, false) => ProcessorExecutionStrategy::MultiStatefulSparse,
        };

        Self {
            processor_id,
            available_axes,
            required_eval_axes,
            required_memory_axes,
            strategy,
        }
    }
}

fn extend_axes(target: &mut AxisSet, source: &AxisSet) {
    target.extend(source.iter().cloned());
}

#[derive(Clone, Copy, Debug, Default)]
pub struct DefaultProcessorContextProvider;

impl ProcessorContextProvider for DefaultProcessorContextProvider {
    fn available_axes(&self, _processor_id: ProcessorId) -> AxisSet {
        AxisSet::new()
    }

    fn iter_context_keys<'a>(
        &'a self,
        _processor_id: ProcessorId,
        axes: &'a AxisSet,
    ) -> Box<dyn Iterator<Item = ContextKey> + 'a> {
        if axes.is_empty() {
            Box::new(std::iter::once(ContextKey::default_lane()))
        } else {
            Box::new(std::iter::empty())
        }
    }

    fn resolve_context_value(
        &self,
        _key: &ContextKey,
        _axis: &ContextAxisId,
        _path: &ContextValuePath,
    ) -> Option<RuntimeValue> {
        None
    }
}

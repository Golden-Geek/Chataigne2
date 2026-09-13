use crate::{ANodeInstance, CompiledNodeEvaluator, ExtensionValue, NodeEvaluation, RuntimeValue, ValueTypeId};
use golden_engine::node::{Curve, CurveEasing, CurveKey};

use super::support::float_inputs;

const CURVE_TYPE: &str = "golden.curve";

#[derive(Debug)]
pub(super) struct CurveRemapEval {
    curve: Curve,
}

impl CurveRemapEval {
    pub(super) fn from_config(instance: &ANodeInstance) -> Result<Self, String> {
        let value = instance
            .config
            .get("curve")
            .cloned()
            .unwrap_or_else(default_curve_config);
        let RuntimeValue::Extension(extension) = value else {
            return Err("Curve Remap requires a Golden curve resource".into());
        };
        if extension.value_type.as_str() != CURVE_TYPE {
            return Err("Curve Remap has the wrong curve resource type".into());
        }
        let curve = serde_json::from_slice(&extension.payload)
            .map_err(|error| format!("invalid Golden curve resource: {error}"))?;
        Ok(Self { curve })
    }
}

impl CompiledNodeEvaluator for CurveRemapEval {
    fn evaluate(&self, evaluation: &mut NodeEvaluation<'_, '_>) -> Result<crate::NodeOutputs, String> {
        let [position] = float_inputs::<1>(evaluation.inputs)?;
        if !position.is_finite() {
            return Err("Curve Remap requires a finite position".into());
        }
        let value = self
            .curve
            .sample(position)
            .ok_or_else(|| "Curve Remap has no sampleable keys".to_owned())?;
        if !value.is_finite() {
            return Err("Curve Remap produced a non-finite value".into());
        }
        Ok(crate::node_outputs![RuntimeValue::Float(value)])
    }
}

pub(super) fn default_curve_config() -> RuntimeValue {
    let curve = Curve::new(vec![
        CurveKey::new(0.0, 0.0, CurveEasing::Linear),
        CurveKey::new(1.0, 1.0, CurveEasing::Linear),
    ]);
    RuntimeValue::Extension(ExtensionValue::new(
        ValueTypeId::new(CURVE_TYPE),
        serde_json::to_vec(&curve).expect("default Golden curve is serializable"),
    ))
}

pub fn curve_config_value(curve: &Curve) -> RuntimeValue {
    RuntimeValue::Extension(ExtensionValue::new(
        ValueTypeId::new(CURVE_TYPE),
        serde_json::to_vec(curve).expect("Golden curve snapshot is serializable"),
    ))
}

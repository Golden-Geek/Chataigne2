use crate::{ANodeInstance, CompiledNodeEvaluator, NodeEvaluation, RuntimeValue};

use super::support::{config_string, float_inputs};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum FunctionKind {
    Sqrt,
    Log,
    Log10,
    Exp,
    Abs,
    Floor,
    Ceil,
    Round,
    Sin,
    Cos,
    Tan,
    Asin,
    Acos,
    Atan,
    Atan2,
}

impl FunctionKind {
    pub(super) fn from_config(instance: &ANodeInstance) -> Self {
        match config_string(instance, "function", "sqrt").as_str() {
            "log" => Self::Log,
            "log10" => Self::Log10,
            "exp" => Self::Exp,
            "abs" => Self::Abs,
            "floor" => Self::Floor,
            "ceil" => Self::Ceil,
            "round" => Self::Round,
            "sin" => Self::Sin,
            "cos" => Self::Cos,
            "tan" => Self::Tan,
            "asin" => Self::Asin,
            "acos" => Self::Acos,
            "atan" => Self::Atan,
            "atan2" => Self::Atan2,
            _ => Self::Sqrt,
        }
    }
}

#[derive(Debug)]
pub(super) struct FunctionEval {
    pub(super) function: FunctionKind,
}

impl CompiledNodeEvaluator for FunctionEval {
    fn evaluate(&self, evaluation: &mut NodeEvaluation<'_, '_>) -> Result<Vec<RuntimeValue>, String> {
        let result = if self.function == FunctionKind::Atan2 {
            let [y, x] = float_inputs::<2>(evaluation.inputs)?;
            y.atan2(x)
        } else {
            let [value] = float_inputs::<1>(evaluation.inputs)?;
            match self.function {
                FunctionKind::Sqrt => value.sqrt(),
                FunctionKind::Log => value.ln(),
                FunctionKind::Log10 => value.log10(),
                FunctionKind::Exp => value.exp(),
                FunctionKind::Abs => value.abs(),
                FunctionKind::Floor => value.floor(),
                FunctionKind::Ceil => value.ceil(),
                FunctionKind::Round => value.round(),
                FunctionKind::Sin => value.sin(),
                FunctionKind::Cos => value.cos(),
                FunctionKind::Tan => value.tan(),
                FunctionKind::Asin => value.asin(),
                FunctionKind::Acos => value.acos(),
                FunctionKind::Atan => value.atan(),
                FunctionKind::Atan2 => unreachable!(),
            }
        };
        Ok(vec![RuntimeValue::Float(result)])
    }
}

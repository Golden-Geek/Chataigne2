use crate::{ANodeInstance, ColorValue, CompiledNodeEvaluator, NodeEvaluation, RuntimeValue};

use super::support::config_string;

#[derive(Clone, Copy, Debug)]
pub(super) enum CompoundTarget {
    Vec2,
    Vec3,
    Color,
}

impl CompoundTarget {
    pub(super) fn from_config(instance: &ANodeInstance) -> Self {
        match config_string(instance, "target", "vec3").as_str() {
            "vec2" => Self::Vec2,
            "color" => Self::Color,
            _ => Self::Vec3,
        }
    }

    pub(super) const fn type_name(self) -> &'static str {
        match self {
            Self::Vec2 => "vec2",
            Self::Vec3 => "vec3",
            Self::Color => "color",
        }
    }
}

#[derive(Debug)]
pub(super) struct ConvertCompoundEval {
    pub(super) target: CompoundTarget,
}

impl CompiledNodeEvaluator for ConvertCompoundEval {
    fn evaluate(&self, evaluation: &mut NodeEvaluation<'_, '_>) -> Result<Vec<RuntimeValue>, String> {
        let [value] = evaluation.inputs else {
            return Err("compound conversion expects one input".into());
        };
        let components = match value {
            RuntimeValue::Vec2([x, y]) => [*x, *y, 0.0, 1.0],
            RuntimeValue::Vec3([x, y, z]) => [*x, *y, *z, 1.0],
            RuntimeValue::Color(color) => [color.red, color.green, color.blue, color.alpha],
            _ => return Err("compound conversion requires Vec2, Vec3, or Color".into()),
        };
        if components.iter().any(|value| !value.is_finite()) {
            return Err("compound conversion requires finite components".into());
        }
        let [x, y, z, alpha] = components;
        let result = match self.target {
            CompoundTarget::Vec2 => RuntimeValue::Vec2([x, y]),
            CompoundTarget::Vec3 => RuntimeValue::Vec3([x, y, z]),
            CompoundTarget::Color => RuntimeValue::Color(ColorValue {
                red: x,
                green: y,
                blue: z,
                alpha,
            }),
        };
        Ok(vec![result])
    }
}

use crate::{ANodeInstance, CompiledNodeEvaluator, NodeEvaluation, RuntimeValue};

use super::support::{config_string, require_inputs};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Comparator {
    Equal,
    NotEqual,
    Greater,
    GreaterOrEqual,
    Less,
    LessOrEqual,
    Longer,
    Shorter,
    Contains,
    Brighter,
    Darker,
}

impl Comparator {
    pub(super) fn from_config(instance: &ANodeInstance) -> Self {
        match config_string(instance, "comparator", "equal").as_str() {
            "not_equal" => Self::NotEqual,
            "greater" => Self::Greater,
            "greater_or_equal" => Self::GreaterOrEqual,
            "less" => Self::Less,
            "less_or_equal" => Self::LessOrEqual,
            "longer" => Self::Longer,
            "shorter" => Self::Shorter,
            "contains" => Self::Contains,
            "brighter" => Self::Brighter,
            "darker" => Self::Darker,
            _ => Self::Equal,
        }
    }
}

#[derive(Debug)]
pub(super) struct CompareEval {
    pub(super) comparator: Comparator,
}

impl CompiledNodeEvaluator for CompareEval {
    fn evaluate(&self, evaluation: &mut NodeEvaluation<'_, '_>) -> Result<Vec<RuntimeValue>, String> {
        let [left, right] = require_inputs::<2>(evaluation.inputs)?;
        if !finite_primitive(left) || !finite_primitive(right) {
            return Err("Compare requires finite values".into());
        }
        let result = match self.comparator {
            Comparator::Equal => left == right,
            Comparator::NotEqual => left != right,
            Comparator::Greater => numeric_order(left, right)?.is_gt(),
            Comparator::GreaterOrEqual => !numeric_order(left, right)?.is_lt(),
            Comparator::Less => numeric_order(left, right)?.is_lt(),
            Comparator::LessOrEqual => !numeric_order(left, right)?.is_gt(),
            Comparator::Longer => {
                string_pair(left, right).map(|(left, right)| left.chars().count() > right.chars().count())?
            }
            Comparator::Shorter => {
                string_pair(left, right).map(|(left, right)| left.chars().count() < right.chars().count())?
            }
            Comparator::Contains => string_pair(left, right).map(|(left, right)| left.contains(right))?,
            Comparator::Brighter => {
                color_pair(left, right).map(|(left, right)| brightness(left) > brightness(right))?
            }
            Comparator::Darker => color_pair(left, right).map(|(left, right)| brightness(left) < brightness(right))?,
        };
        Ok(vec![RuntimeValue::Bool(result)])
    }
}

fn numeric_order(left: &RuntimeValue, right: &RuntimeValue) -> Result<std::cmp::Ordering, String> {
    match (left, right) {
        (RuntimeValue::Int(left), RuntimeValue::Int(right)) => Ok(left.cmp(right)),
        (RuntimeValue::Float(left), RuntimeValue::Float(right)) => left
            .partial_cmp(right)
            .ok_or_else(|| "Compare requires finite numbers".into()),
        _ => Err("ordered comparison requires matching integer or float inputs".into()),
    }
}

fn string_pair<'a>(left: &'a RuntimeValue, right: &'a RuntimeValue) -> Result<(&'a str, &'a str), String> {
    match (left, right) {
        (RuntimeValue::String(left), RuntimeValue::String(right)) => Ok((left, right)),
        _ => Err("string comparison requires two strings".into()),
    }
}

fn color_pair<'a>(
    left: &'a RuntimeValue,
    right: &'a RuntimeValue,
) -> Result<(&'a crate::ColorValue, &'a crate::ColorValue), String> {
    match (left, right) {
        (RuntimeValue::Color(left), RuntimeValue::Color(right)) => Ok((left, right)),
        _ => Err("brightness comparison requires two colors".into()),
    }
}

fn brightness(color: &crate::ColorValue) -> f64 {
    0.2126 * color.red + 0.7152 * color.green + 0.0722 * color.blue
}

fn finite_primitive(value: &RuntimeValue) -> bool {
    match value {
        RuntimeValue::Float(value) => value.is_finite(),
        RuntimeValue::Vec2(value) => value.iter().all(|component| component.is_finite()),
        RuntimeValue::Vec3(value) => value.iter().all(|component| component.is_finite()),
        RuntimeValue::Color(value) => [value.red, value.green, value.blue, value.alpha]
            .iter()
            .all(|component| component.is_finite()),
        _ => true,
    }
}

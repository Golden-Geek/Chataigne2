use std::time::Duration;

use golden_values::{StableRef, Value};
use indexmap::IndexMap;

use crate::{
    CompiledConditionInstruction, CompiledConditionProgram, ConditionBehavior, ConditionGroupPolicy, ConditionId,
    ConditionOperand, ConditionProjection, ConditionStateKey, EdgePolicy, TypedComparator,
};

pub trait ConditionInputProvider {
    fn input_value(&self, input: &StableRef) -> Option<Value>;

    fn input_node_value(&self, _provider: &str, _node: &StableRef) -> Option<Value> {
        None
    }

    fn script_condition(&self, _script: &str) -> Result<bool, String> {
        Err("script condition provider is unavailable".to_owned())
    }
}

pub struct ConditionEvaluationFrame<'a> {
    pub logical_tick: u64,
    pub delta_time: Duration,
    pub inputs: &'a dyn ConditionInputProvider,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ConditionState {
    pub previous_value: Option<Value>,
    pub previous_level: bool,
    pub latched: bool,
    pub transient_until: Option<u64>,
    pub pending_level: Option<bool>,
    pub pending_elapsed: Duration,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConditionEvaluationResult {
    pub value: bool,
    pub observations: IndexMap<ConditionId, bool>,
}

#[derive(Clone, Debug, thiserror::Error, PartialEq)]
pub enum ConditionEvaluationError {
    #[error("condition input `{0:?}` is unavailable")]
    MissingInput(StableRef),
    #[error("condition node `{node:?}` from provider `{provider}` is unavailable")]
    MissingNode { provider: String, node: StableRef },
    #[error("condition projection `{projection:?}` is invalid for `{value:?}`")]
    InvalidProjection {
        projection: ConditionProjection,
        value: Value,
    },
    #[error("condition comparator `{comparator:?}` cannot compare `{actual:?}` with `{expected:?}`")]
    InvalidComparison {
        comparator: TypedComparator,
        actual: Value,
        expected: Value,
    },
    #[error("script condition `{script}` failed: {message}")]
    Script { script: String, message: String },
}

#[derive(Clone, Debug, Default)]
pub struct ConditionRuntime {
    keys: Vec<ConditionStateKey>,
    states: Vec<ConditionState>,
    instruction_values: Vec<bool>,
}

impl ConditionRuntime {
    #[must_use]
    pub fn new(program: &CompiledConditionProgram) -> Self {
        Self {
            keys: program.state_layout.keys.clone(),
            states: vec![ConditionState::default(); program.state_layout.keys.len()],
            instruction_values: Vec::with_capacity(program.instructions.len()),
        }
    }

    #[must_use]
    pub fn migrate(&self, program: &CompiledConditionProgram) -> Self {
        let previous: IndexMap<_, _> = self.keys.iter().cloned().zip(self.states.iter().cloned()).collect();
        let keys = program.state_layout.keys.clone();
        let states = keys
            .iter()
            .map(|key| previous.get(key).cloned().unwrap_or_default())
            .collect();
        Self {
            keys,
            states,
            instruction_values: Vec::with_capacity(program.instructions.len()),
        }
    }

    #[must_use]
    pub fn state_keys(&self) -> &[ConditionStateKey] {
        &self.keys
    }

    pub fn evaluate(
        &mut self,
        program: &CompiledConditionProgram,
        frame: &ConditionEvaluationFrame<'_>,
    ) -> Result<ConditionEvaluationResult, ConditionEvaluationError> {
        self.evaluate_instructions(program, frame)?;
        let mut observations = IndexMap::with_capacity(program.observation.len());
        for descriptor in &program.observation {
            observations.insert(descriptor.condition, self.instruction_values[descriptor.instruction]);
        }
        Ok(ConditionEvaluationResult {
            value: self.instruction_values[program.root],
            observations,
        })
    }

    /// Evaluates the condition without materializing per-condition observations.
    ///
    /// Use this on the runtime hot path when only the root result is needed. The
    /// instruction buffer is retained by the runtime and reused across calls.
    pub fn evaluate_value(
        &mut self,
        program: &CompiledConditionProgram,
        frame: &ConditionEvaluationFrame<'_>,
    ) -> Result<bool, ConditionEvaluationError> {
        self.evaluate_instructions(program, frame)?;
        Ok(self.instruction_values[program.root])
    }

    fn evaluate_instructions(
        &mut self,
        program: &CompiledConditionProgram,
        frame: &ConditionEvaluationFrame<'_>,
    ) -> Result<(), ConditionEvaluationError> {
        if self.keys != program.state_layout.keys {
            *self = self.migrate(program);
        }
        self.instruction_values.clear();
        self.instruction_values.reserve(program.instructions.len());
        let states = &mut self.states;
        let values = &mut self.instruction_values;
        for instruction in &program.instructions {
            let value =
                match instruction {
                    CompiledConditionInstruction::Constant(value) => *value,
                    CompiledConditionInstruction::InputValue {
                        input,
                        projection,
                        comparator,
                        expected,
                        expected_max,
                        behavior,
                        state_slot,
                    } => {
                        let actual = frame
                            .inputs
                            .input_value(input)
                            .ok_or_else(|| ConditionEvaluationError::MissingInput(input.clone()))?;
                        let expected = resolve_operand(expected, frame.inputs)?;
                        let expected_max = expected_max
                            .as_ref()
                            .map(|operand| resolve_operand(operand, frame.inputs))
                            .transpose()?;
                        evaluate_leaf(
                            actual,
                            *projection,
                            *comparator,
                            &expected,
                            expected_max.as_ref(),
                            *behavior,
                            &mut states[*state_slot],
                            frame,
                        )?
                    }
                    CompiledConditionInstruction::InputNode {
                        provider,
                        node,
                        projection,
                        comparator,
                        expected,
                        expected_max,
                        behavior,
                        state_slot,
                    } => {
                        let actual = frame.inputs.input_node_value(provider, node).ok_or_else(|| {
                            ConditionEvaluationError::MissingNode {
                                provider: provider.clone(),
                                node: node.clone(),
                            }
                        })?;
                        let expected = resolve_operand(expected, frame.inputs)?;
                        let expected_max = expected_max
                            .as_ref()
                            .map(|operand| resolve_operand(operand, frame.inputs))
                            .transpose()?;
                        evaluate_leaf(
                            actual,
                            *projection,
                            *comparator,
                            &expected,
                            expected_max.as_ref(),
                            *behavior,
                            &mut states[*state_slot],
                            frame,
                        )?
                    }
                    CompiledConditionInstruction::Group { policy, children } => match policy {
                        ConditionGroupPolicy::All => children.iter().all(|child| values[*child]),
                        ConditionGroupPolicy::Any => children.iter().any(|child| values[*child]),
                        ConditionGroupPolicy::None => children.iter().all(|child| !values[*child]),
                        ConditionGroupPolicy::AtLeast(count) => {
                            children.iter().filter(|child| values[**child]).count() >= *count
                        }
                        ConditionGroupPolicy::Exactly(count) => {
                            children.iter().filter(|child| values[**child]).count() == *count
                        }
                    },
                    CompiledConditionInstruction::Script {
                        script,
                        behavior,
                        state_slot,
                    } => {
                        let level = frame.inputs.script_condition(script).map_err(|message| {
                            ConditionEvaluationError::Script {
                                script: script.clone(),
                                message,
                            }
                        })?;
                        apply_behavior(
                            level,
                            *behavior,
                            &mut states[*state_slot],
                            frame.logical_tick,
                            frame.delta_time,
                        )
                    }
                };
            values.push(value);
        }
        Ok(())
    }
}

fn resolve_operand(
    operand: &ConditionOperand,
    inputs: &dyn ConditionInputProvider,
) -> Result<Value, ConditionEvaluationError> {
    match operand {
        ConditionOperand::Literal(value) => Ok(value.clone()),
        ConditionOperand::Input(input) => inputs
            .input_value(input)
            .ok_or_else(|| ConditionEvaluationError::MissingInput(input.clone())),
    }
}

fn evaluate_leaf(
    actual: Value,
    projection: ConditionProjection,
    comparator: TypedComparator,
    expected: &Value,
    expected_max: Option<&Value>,
    behavior: ConditionBehavior,
    state: &mut ConditionState,
    frame: &ConditionEvaluationFrame<'_>,
) -> Result<bool, ConditionEvaluationError> {
    let projected = project(actual, projection, state, frame.delta_time)?;
    let level = compare(
        &projected,
        comparator,
        expected,
        expected_max,
        state.previous_value.as_ref(),
    )?;
    let result = apply_behavior(level, behavior, state, frame.logical_tick, frame.delta_time);
    state.previous_value = Some(projected);
    Ok(result)
}

fn apply_behavior(
    level: bool,
    behavior: ConditionBehavior,
    state: &mut ConditionState,
    logical_tick: u64,
    delta_time: Duration,
) -> bool {
    let required_delay_ms = if level {
        behavior.validation_delay_ms
    } else {
        behavior.invalidation_delay_ms
    };
    let level = if level == state.previous_level || required_delay_ms == 0 {
        state.pending_level = None;
        state.pending_elapsed = Duration::ZERO;
        level
    } else {
        if state.pending_level == Some(level) {
            state.pending_elapsed = state.pending_elapsed.saturating_add(delta_time);
        } else {
            state.pending_level = Some(level);
            state.pending_elapsed = delta_time;
        }
        if state.pending_elapsed.as_millis() >= u128::from(required_delay_ms) {
            state.pending_level = None;
            state.pending_elapsed = Duration::ZERO;
            level
        } else {
            state.previous_level
        }
    };
    let pulse = match behavior.edge {
        EdgePolicy::Level => level,
        EdgePolicy::Rising => level && !state.previous_level,
        EdgePolicy::Falling => !level && state.previous_level,
        EdgePolicy::Either => level != state.previous_level,
    };
    state.previous_level = level;
    let mut result = pulse;
    if behavior.toggle {
        if pulse {
            state.latched = !state.latched;
        }
        result = state.latched;
    }
    if behavior.transient_ticks > 0 {
        if result {
            state.transient_until = Some(logical_tick.saturating_add(u64::from(behavior.transient_ticks)));
        }
        result = state.transient_until.is_some_and(|until| logical_tick <= until);
    }
    result
}

fn project(
    value: Value,
    projection: ConditionProjection,
    state: &ConditionState,
    delta_time: Duration,
) -> Result<Value, ConditionEvaluationError> {
    let invalid = |value| ConditionEvaluationError::InvalidProjection { projection, value };
    match (projection, value) {
        (ConditionProjection::Identity, value) => Ok(value),
        (ConditionProjection::Vec2Magnitude, Value::Vec2(value)) => Ok(Value::Float(value[0].hypot(value[1]))),
        (ConditionProjection::Vec2X, Value::Vec2(value)) => Ok(Value::Float(value[0])),
        (ConditionProjection::Vec2Y, Value::Vec2(value)) => Ok(Value::Float(value[1])),
        (ConditionProjection::Vec3Magnitude, Value::Vec3(value)) => Ok(Value::Float(
            (value[0].powi(2) + value[1].powi(2) + value[2].powi(2)).sqrt(),
        )),
        (ConditionProjection::Vec3X, Value::Vec3(value)) => Ok(Value::Float(value[0])),
        (ConditionProjection::Vec3Y, Value::Vec3(value)) => Ok(Value::Float(value[1])),
        (ConditionProjection::Vec3Z, Value::Vec3(value)) => Ok(Value::Float(value[2])),
        (ConditionProjection::ColorLuminance, Value::Color(value)) => Ok(Value::Float(
            value.red * 0.2126 + value.green * 0.7152 + value.blue * 0.0722,
        )),
        (ConditionProjection::ColorRed, Value::Color(value)) => Ok(Value::Float(value.red)),
        (ConditionProjection::ColorGreen, Value::Color(value)) => Ok(Value::Float(value.green)),
        (ConditionProjection::ColorBlue, Value::Color(value)) => Ok(Value::Float(value.blue)),
        (ConditionProjection::ColorAlpha, Value::Color(value)) => Ok(Value::Float(value.alpha)),
        (ConditionProjection::StringLength, Value::String(value)) => {
            Ok(Value::Int(i64::try_from(value.chars().count()).unwrap_or(i64::MAX)))
        }
        (ConditionProjection::DurationSeconds, Value::Duration(value)) => Ok(Value::Float(value.as_secs_f64())),
        (ConditionProjection::Speed | ConditionProjection::AbsoluteSpeed, value) => {
            let current = number(&value).ok_or_else(|| invalid(value.clone()))?;
            let previous = state.previous_value.as_ref().and_then(number).unwrap_or(current);
            let seconds = delta_time.as_secs_f64();
            let speed = if seconds > 0.0 {
                (current - previous) / seconds
            } else {
                0.0
            };
            Ok(Value::Float(if projection == ConditionProjection::AbsoluteSpeed {
                speed.abs()
            } else {
                speed
            }))
        }
        (_, value) => Err(invalid(value)),
    }
}

fn compare(
    actual: &Value,
    comparator: TypedComparator,
    expected: &Value,
    expected_max: Option<&Value>,
    previous: Option<&Value>,
) -> Result<bool, ConditionEvaluationError> {
    let invalid = || ConditionEvaluationError::InvalidComparison {
        comparator,
        actual: actual.clone(),
        expected: expected.clone(),
    };
    match comparator {
        TypedComparator::Equal => Ok(actual == expected),
        TypedComparator::NotEqual => Ok(actual != expected),
        TypedComparator::Changed => Ok(previous.is_some_and(|previous| previous != actual)),
        TypedComparator::Triggered => match actual {
            Value::Trigger(value) => Ok(value.fired),
            _ => Err(invalid()),
        },
        TypedComparator::Greater
        | TypedComparator::GreaterOrEqual
        | TypedComparator::Less
        | TypedComparator::LessOrEqual => {
            let left = number(actual).ok_or_else(invalid)?;
            let right = number(expected).ok_or_else(invalid)?;
            Ok(match comparator {
                TypedComparator::Greater => left > right,
                TypedComparator::GreaterOrEqual => left >= right,
                TypedComparator::Less => left < right,
                TypedComparator::LessOrEqual => left <= right,
                _ => unreachable!(),
            })
        }
        TypedComparator::Between | TypedComparator::Outside => {
            let left = number(actual).ok_or_else(invalid)?;
            let first = number(expected).ok_or_else(invalid)?;
            let second = expected_max.and_then(number).ok_or_else(invalid)?;
            let minimum = first.min(second);
            let maximum = first.max(second);
            let between = left >= minimum && left <= maximum;
            Ok(if comparator == TypedComparator::Between {
                between
            } else {
                !between
            })
        }
        TypedComparator::Contains
        | TypedComparator::DoesNotContain
        | TypedComparator::StartsWith
        | TypedComparator::EndsWith
        | TypedComparator::RegexMatch => {
            let (Value::String(left), Value::String(right)) = (actual, expected) else {
                return Err(invalid());
            };
            Ok(match comparator {
                TypedComparator::Contains => left.contains(right.as_ref()),
                TypedComparator::DoesNotContain => !left.contains(right.as_ref()),
                TypedComparator::StartsWith => left.starts_with(right.as_ref()),
                TypedComparator::EndsWith => left.ends_with(right.as_ref()),
                TypedComparator::RegexMatch => regex::Regex::new(right).is_ok_and(|regex| regex.is_match(left)),
                _ => unreachable!(),
            })
        }
    }
}

fn number(value: &Value) -> Option<f64> {
    match value {
        Value::Int(value) => Some(*value as f64),
        Value::Float(value) => Some(*value),
        Value::Duration(value) => Some(value.as_secs_f64()),
        _ => None,
    }
}

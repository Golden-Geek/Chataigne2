use super::*;

pub(super) fn runtime_node_inputs_into(
    node: &CompiledExecNode,
    memory: &mut AlchemistMemory,
    ctx: &EvaluationCtx<'_>,
) -> Result<(), String> {
    memory.runtime_inputs.clear();
    for source in &node.inputs {
        let value = runtime_input_value(source, memory, ctx.inputs, ctx.registries.value_types)?;
        memory.runtime_inputs.push(value);
    }
    Ok(())
}

pub(super) fn change_detection_inputs_into(
    change_inputs: &mut Vec<RuntimeValue>,
    operation: &CompiledNodeOperation,
    inputs: &[RuntimeValue],
    properties: &RuntimePropertyFrame,
    ctx: &EvaluationCtx<'_>,
    context: &RuntimeContextFrame,
) -> Result<(), String> {
    change_inputs.clear();
    change_inputs.extend_from_slice(inputs);
    if let CompiledNodeOperation::ReadProperty(slot) = operation {
        change_inputs.push(
            properties
                .get(*slot)
                .cloned()
                .ok_or_else(|| format!("property slot {} is unavailable", slot.index()))?,
        );
    }
    if let CompiledNodeOperation::Custom(evaluator) = operation {
        change_inputs.extend(evaluator.change_detection_inputs(ctx, context)?);
    }
    Ok(())
}

pub(super) fn runtime_input_value(
    source: &InputValueSource,
    memory: &AlchemistMemory,
    inputs: &RuntimeInputSnapshot,
    value_types: &ValueTypeRegistry,
) -> Result<RuntimeValue, String> {
    match source {
        InputValueSource::Slot(slot) => Ok(memory.values[slot.index()].clone()),
        InputValueSource::Converted { source, target_type } => {
            let value = runtime_input_value(source, memory, inputs, value_types)?;
            value_types.convert_automatically(&value, target_type)
        }
        InputValueSource::Component { source, component } => {
            let value = runtime_input_value(source, memory, inputs, value_types)?;
            value
                .component(*component)
                .ok_or_else(|| format!("value type `{}` has no component `{component:?}`", value.value_type()))
        }
        InputValueSource::Composite {
            target_type,
            base,
            components,
        } => {
            let base = runtime_input_value(base, memory, inputs, value_types)?;
            let mut value = value_types.convert_automatically(&base, target_type)?;
            for (component, source) in components {
                let component_value = runtime_input_value(source, memory, inputs, value_types)?;
                value = value.with_component(*component, &component_value)?;
            }
            Ok(value)
        }
        InputValueSource::RuntimeInput { reference, fallback } => inputs
            .get(reference)
            .cloned()
            .map(Ok)
            .unwrap_or_else(|| runtime_input_value(fallback, memory, inputs, value_types)),
        InputValueSource::Constant(value) => Ok(value.clone()),
        InputValueSource::Unset => Ok(RuntimeValue::Unit),
    }
}

pub(super) fn evaluate_operation(
    operation: &CompiledNodeOperation,
    mut evaluation: NodeEvaluation<'_, '_>,
) -> Result<SmallVec<[RuntimeValue; 4]>, String> {
    match operation {
        CompiledNodeOperation::Disabled { outputs } => Ok(outputs
            .iter()
            .map(|output| {
                output
                    .input_index
                    .and_then(|input_index| evaluation.inputs.get(input_index))
                    .cloned()
                    .unwrap_or_else(|| output.default_value.clone())
            })
            .collect()),
        CompiledNodeOperation::Constant(value) => Ok(smallvec::smallvec![value.clone()]),
        CompiledNodeOperation::ReadProperty(slot) => evaluation
            .properties
            .get(*slot)
            .cloned()
            .map(|value| smallvec::smallvec![value])
            .ok_or_else(|| format!("property slot {} is unavailable", slot.index())),
        CompiledNodeOperation::Add => {
            let [left, right] = require_inputs::<2>(evaluation.inputs)?;
            Ok(smallvec::smallvec![add_values(left, right)?])
        }
        CompiledNodeOperation::Compare => {
            let [left, right] = require_inputs::<2>(evaluation.inputs)?;
            Ok(smallvec::smallvec![RuntimeValue::Bool(left == right)])
        }
        CompiledNodeOperation::BoolAnd => {
            let [left, right] = bool_inputs::<2>(evaluation.inputs)?;
            Ok(smallvec::smallvec![RuntimeValue::Bool(left && right)])
        }
        CompiledNodeOperation::BoolOr => {
            let [left, right] = bool_inputs::<2>(evaluation.inputs)?;
            Ok(smallvec::smallvec![RuntimeValue::Bool(left || right)])
        }
        CompiledNodeOperation::BoolNot => {
            let [value] = bool_inputs::<1>(evaluation.inputs)?;
            Ok(smallvec::smallvec![RuntimeValue::Bool(!value)])
        }
        CompiledNodeOperation::Edge => {
            let [value] = bool_inputs::<1>(evaluation.inputs)?;
            let previous = matches!(evaluation.state.first(), Some(RuntimeValue::Bool(true)));
            if let Some(state) = evaluation.state.first_mut() {
                *state = RuntimeValue::Bool(value);
            }
            Ok(smallvec::smallvec![RuntimeValue::Trigger(TriggerValue {
                fired: value && !previous,
                edge_id: u64::from(evaluation.exec_node.index() as u32),
                logical_tick: evaluation.ctx.logical_tick,
            })])
        }
        CompiledNodeOperation::Gate => {
            let [trigger, open] = require_inputs::<2>(evaluation.inputs)?;
            let RuntimeValue::Trigger(trigger) = trigger else {
                return Err("Gate expects a trigger input".into());
            };
            let RuntimeValue::Bool(open) = open else {
                return Err("Gate expects a boolean open input".into());
            };
            Ok(smallvec::smallvec![RuntimeValue::Trigger(TriggerValue {
                fired: trigger.fired && *open,
                ..*trigger
            })])
        }
        CompiledNodeOperation::MapRange => Ok(smallvec::smallvec![map_range_values(evaluation.inputs)?]),
        CompiledNodeOperation::Clamp => Ok(smallvec::smallvec![clamp_values(evaluation.inputs)?]),
        CompiledNodeOperation::DelayOneTick => {
            let [value] = require_inputs::<1>(evaluation.inputs)?;
            let [initialized, previous] = evaluation.state else {
                return Err("Delay One Tick requires two runtime state slots".into());
            };
            let output = if matches!(initialized, RuntimeValue::Bool(true)) {
                previous.clone()
            } else {
                value.clone()
            };
            *initialized = RuntimeValue::Bool(true);
            *previous = value.clone();
            Ok(smallvec::smallvec![output])
        }
        CompiledNodeOperation::DebugLog => {
            let [value] = require_inputs::<1>(evaluation.inputs)?;
            evaluation.intents.push(RuntimeIntent {
                kind: Arc::from("debug.log"),
                source_node: Some(evaluation.author_node_id),
                source_socket: None,
                target: None,
                payload: value.clone(),
                logical_tick: evaluation.ctx.logical_tick,
            });
            Ok(SmallVec::new())
        }
        CompiledNodeOperation::Custom(evaluator) => evaluator.evaluate(&mut evaluation),
    }
}

fn require_inputs<const N: usize>(inputs: &[RuntimeValue]) -> Result<[&RuntimeValue; N], String> {
    inputs
        .try_into()
        .map(<[RuntimeValue; N]>::each_ref)
        .map_err(|_| format!("node expects {N} input(s)"))
}

fn bool_inputs<const N: usize>(inputs: &[RuntimeValue]) -> Result<[bool; N], String> {
    require_inputs::<N>(inputs)?
        .map(|value| match value {
            RuntimeValue::Bool(value) => Ok(*value),
            _ => Err("node expects boolean inputs".into()),
        })
        .into_iter()
        .collect::<Result<Vec<_>, String>>()?
        .try_into()
        .map_err(|_| "invalid boolean input count".into())
}

fn add_values(left: &RuntimeValue, right: &RuntimeValue) -> Result<RuntimeValue, String> {
    match (left, right) {
        (RuntimeValue::Int(left), RuntimeValue::Int(right)) => Ok(RuntimeValue::Int(left + right)),
        (RuntimeValue::Float(left), RuntimeValue::Float(right)) => Ok(RuntimeValue::Float(left + right)),
        (RuntimeValue::Int(left), RuntimeValue::Float(right)) => Ok(RuntimeValue::Float(*left as f64 + right)),
        (RuntimeValue::Float(left), RuntimeValue::Int(right)) => Ok(RuntimeValue::Float(left + *right as f64)),
        (RuntimeValue::Vec2(left), RuntimeValue::Vec2(right)) => {
            Ok(RuntimeValue::Vec2([left[0] + right[0], left[1] + right[1]]))
        }
        (RuntimeValue::Vec3(left), RuntimeValue::Vec3(right)) => Ok(RuntimeValue::Vec3([
            left[0] + right[0],
            left[1] + right[1],
            left[2] + right[2],
        ])),
        (RuntimeValue::Color(left), RuntimeValue::Color(right)) => Ok(RuntimeValue::Color(ColorValue {
            red: left.red + right.red,
            green: left.green + right.green,
            blue: left.blue + right.blue,
            alpha: left.alpha + right.alpha,
        })),
        _ => Err("Add received incompatible runtime values".into()),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NumericShape {
    Int,
    Scalar,
    Vec2,
    Vec3,
    Color,
}

fn numeric_components(value: &RuntimeValue) -> Result<(NumericShape, Vec<f64>), String> {
    match value {
        RuntimeValue::Int(value) => Ok((NumericShape::Int, vec![*value as f64])),
        RuntimeValue::Float(value) => Ok((NumericShape::Scalar, vec![*value])),
        RuntimeValue::Vec2(value) => Ok((NumericShape::Vec2, value.to_vec())),
        RuntimeValue::Vec3(value) => Ok((NumericShape::Vec3, value.to_vec())),
        RuntimeValue::Color(value) => Ok((
            NumericShape::Color,
            vec![value.red, value.green, value.blue, value.alpha],
        )),
        _ => Err("node expects numeric inputs".into()),
    }
}

fn numeric_from_components(shape: NumericShape, components: &[f64]) -> RuntimeValue {
    match shape {
        NumericShape::Int => RuntimeValue::Int(components[0] as i64),
        NumericShape::Scalar => RuntimeValue::Float(components[0]),
        NumericShape::Vec2 => RuntimeValue::Vec2([components[0], components[1]]),
        NumericShape::Vec3 => RuntimeValue::Vec3([components[0], components[1], components[2]]),
        NumericShape::Color => RuntimeValue::Color(ColorValue {
            red: components[0],
            green: components[1],
            blue: components[2],
            alpha: components[3],
        }),
    }
}

fn aligned_numeric_components(values: &[RuntimeValue]) -> Result<(NumericShape, Vec<Vec<f64>>), String> {
    let mut components = values.iter().map(numeric_components).collect::<Result<Vec<_>, _>>()?;
    let Some((shape, first)) = components.first() else {
        return Err("node expects numeric inputs".into());
    };
    let shape = *shape;
    let count = first.len();
    if components
        .iter()
        .any(|(candidate_shape, values)| *candidate_shape != shape || values.len() != count)
    {
        return Err("node received incompatible numeric shapes".into());
    }
    Ok((shape, components.drain(..).map(|(_, components)| components).collect()))
}

fn map_range_values(inputs: &[RuntimeValue]) -> Result<RuntimeValue, String> {
    let values = require_inputs::<5>(inputs)?.map(Clone::clone);
    let (shape, values) = aligned_numeric_components(&values)?;
    let [value, in_min, in_max, out_min, out_max] = values
        .try_into()
        .map_err(|_| "invalid Map Range input count".to_string())?;
    let mut result = Vec::with_capacity(value.len());
    for index in 0..value.len() {
        if (in_max[index] - in_min[index]).abs() <= f64::EPSILON {
            return Err("Map Range input range cannot be zero".into());
        }
        let normalized = (value[index] - in_min[index]) / (in_max[index] - in_min[index]);
        result.push(out_min[index] + normalized * (out_max[index] - out_min[index]));
    }
    Ok(numeric_from_components(shape, &result))
}

fn clamp_values(inputs: &[RuntimeValue]) -> Result<RuntimeValue, String> {
    let values = require_inputs::<3>(inputs)?;
    if let [
        RuntimeValue::Int(value),
        RuntimeValue::Int(minimum),
        RuntimeValue::Int(maximum),
    ] = values
    {
        if minimum > maximum {
            return Err("Clamp minimum cannot exceed maximum".into());
        }
        return Ok(RuntimeValue::Int((*value).clamp(*minimum, *maximum)));
    }
    let values = values.map(Clone::clone);
    let (shape, values) = aligned_numeric_components(&values)?;
    let [value, minimum, maximum] = values.try_into().map_err(|_| "invalid Clamp input count".to_string())?;
    let mut result = Vec::with_capacity(value.len());
    for (value, (minimum, maximum)) in value.iter().zip(minimum.iter().zip(maximum.iter())) {
        if !value.is_finite() || !minimum.is_finite() || !maximum.is_finite() {
            return Err("Clamp requires finite inputs".into());
        }
        if minimum > maximum {
            return Err("Clamp minimum cannot exceed maximum".into());
        }
        result.push(value.clamp(*minimum, *maximum));
    }
    Ok(numeric_from_components(shape, &result))
}

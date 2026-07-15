use std::{collections::HashMap, sync::Arc};

use golden_condition::{
    CompiledConditionProgram, ConditionBehavior, ConditionDefinition, ConditionGroupPolicy,
    ConditionId, ConditionKind, ConditionOperand, ConditionProjection, EdgePolicy,
    InputNodeCondition, InputValueCondition, ScriptCondition, TypedComparator, compile_condition,
};
use golden_core::{
    node::NodeId,
    parameter::ParamValue,
    process_ctx::ProcessTreeSnapshot,
};
use golden_values::{StableRef, Value, ValueTypeId};

const CONDITION_GROUP_NODE_TYPE: &str = "sm_condition_group";
const CONDITION_MANAGER_NODE_TYPE: &str = "sm_condition_manager";
const INPUT_NODE_CONDITION_NODE_TYPE: &str = "sm_input_node_condition";
const INPUT_VALUE_CONDITION_NODE_TYPE: &str = "sm_input_value_condition";
const SCRIPT_CONDITION_NODE_TYPE: &str = "sm_script_condition";
pub(crate) const CONDITION_SOURCE_REF_TYPE: &str = "chataigne.condition.source";
pub(crate) const CONDITION_OPERAND_REF_TYPE: &str = "chataigne.condition.operand";

#[derive(Clone)]
pub(crate) struct CompiledManagerCondition {
    pub current: Arc<CompiledConditionProgram>,
    pub settled: Arc<CompiledConditionProgram>,
    pub bindings: Arc<HashMap<StableRef, ConditionBinding>>,
}

#[derive(Clone)]
pub(crate) enum ConditionBinding {
    Source(NodeId),
    Param(NodeId),
    Constant(Value),
}

pub(crate) fn compile_manager_condition(
    snapshot: &ProcessTreeSnapshot,
    root: NodeId,
) -> Result<CompiledManagerCondition, Vec<String>> {
    let mut bindings = HashMap::new();
    let current = definition(snapshot, root, false, &mut bindings).ok_or_else(|| {
        vec![format!("condition manager `{root:?}` has no compilable condition tree")]
    })?;
    let settled = definition(snapshot, root, true, &mut bindings).ok_or_else(|| {
        vec![format!("condition manager `{root:?}` has no settled condition tree")]
    })?;
    let current = compile_condition(&current).map_err(diagnostic_messages)?;
    let settled = compile_condition(&settled).map_err(diagnostic_messages)?;
    Ok(CompiledManagerCondition {
        current: Arc::new(current),
        settled: Arc::new(settled),
        bindings: Arc::new(bindings),
    })
}

fn diagnostic_messages(
    diagnostics: Vec<golden_condition::ConditionCompileDiagnostic>,
) -> Vec<String> {
    diagnostics
        .into_iter()
        .map(|diagnostic| diagnostic.message)
        .collect()
}

fn definition(
    snapshot: &ProcessTreeSnapshot,
    node_id: NodeId,
    settled: bool,
    bindings: &mut HashMap<StableRef, ConditionBinding>,
) -> Option<ConditionDefinition> {
    let node = snapshot.node(node_id)?;
    let id = ConditionId::from_uuid(node.uuid.0);
    let kind = match node.node_type.as_str() {
        CONDITION_MANAGER_NODE_TYPE | CONDITION_GROUP_NODE_TYPE => {
            let children = snapshot
                .child_ids(node_id)
                .into_iter()
                .filter_map(|child| {
                    let child_node = snapshot.node(child)?;
                    (child_node.param_value.is_none() && child_node.enabled)
                        .then(|| definition(snapshot, child, settled, bindings))
                        .flatten()
                })
                .collect::<Vec<_>>();
            let operator = child_string(snapshot, node_id, "operator")
                .unwrap_or_else(|| "all".to_owned());
            let count = child_number(snapshot, node_id, "operator_count")
                .unwrap_or(1.0)
                .max(0.0) as usize;
            if children.is_empty() {
                ConditionKind::Constant(true)
            } else {
                ConditionKind::Group {
                    policy: match operator.as_str() {
                        "any" => ConditionGroupPolicy::Any,
                        "none" => ConditionGroupPolicy::None,
                        "at_least" => ConditionGroupPolicy::AtLeast(count),
                        "exactly" => ConditionGroupPolicy::Exactly(count),
                        _ => ConditionGroupPolicy::All,
                    },
                    children,
                }
            }
        }
        INPUT_VALUE_CONDITION_NODE_TYPE => {
            let comparator_name = child_string(snapshot, node_id, "comparator")
                .unwrap_or_else(|| "equal".to_owned());
            if settled && comparator_is_transient(snapshot, node_id, &comparator_name) {
                ConditionKind::Constant(false)
            } else {
                let (projection, comparator) = comparison_plan(
                    snapshot,
                    node_id,
                    child_string(snapshot, node_id, "source_projection").as_deref(),
                    &comparator_name,
                );
                let input = bind_source(
                    snapshot,
                    node_id,
                    "source",
                    node.uuid.0,
                    bindings,
                )?;
                let expected = bind_expected(
                    snapshot,
                    node_id,
                    &comparator_name,
                    node.uuid.0,
                    bindings,
                )?;
                let expected_max = matches!(
                    comparator,
                    TypedComparator::Between | TypedComparator::Outside
                )
                .then(|| {
                    bind_param(
                        snapshot,
                        node_id,
                        "reference_max",
                        node.uuid.0,
                        bindings,
                    )
                    .map(ConditionOperand::Input)
                })
                .flatten();
                ConditionKind::InputValue(InputValueCondition {
                    input,
                    projection,
                    comparator,
                    expected,
                    expected_max,
                    behavior: behavior(snapshot, node_id),
                })
            }
        }
        INPUT_NODE_CONDITION_NODE_TYPE => {
            let comparator_name = child_string(snapshot, node_id, "comparator")
                .unwrap_or_else(|| "equal".to_owned());
            if settled && comparator_name == "value_changed" {
                ConditionKind::Constant(false)
            } else {
                let (_, comparator) = comparison_plan(snapshot, node_id, None, &comparator_name);
                let node_ref = bind_source(
                    snapshot,
                    node_id,
                    "provider_node",
                    node.uuid.0,
                    bindings,
                )?;
                let expected = bind_expected(
                    snapshot,
                    node_id,
                    &comparator_name,
                    node.uuid.0,
                    bindings,
                )?;
                let expected_max = matches!(
                    comparator,
                    TypedComparator::Between | TypedComparator::Outside
                )
                .then(|| {
                    bind_param(
                        snapshot,
                        node_id,
                        "reference_max",
                        node.uuid.0,
                        bindings,
                    )
                    .map(ConditionOperand::Input)
                })
                .flatten();
                ConditionKind::InputNode(InputNodeCondition {
                    provider: child_string(snapshot, node_id, "endpoint_id").unwrap_or_default(),
                    node: node_ref,
                    projection: ConditionProjection::Identity,
                    comparator,
                    expected,
                    expected_max,
                    behavior: behavior(snapshot, node_id),
                })
            }
        }
        SCRIPT_CONDITION_NODE_TYPE => ConditionKind::Script(ScriptCondition {
            script: child_string(snapshot, node_id, "script").unwrap_or_default(),
            behavior: behavior(snapshot, node_id),
        }),
        _ => return None,
    };
    Some(ConditionDefinition {
        id,
        label: node.label.clone(),
        enabled: node.enabled,
        kind,
    })
}

fn behavior(snapshot: &ProcessTreeSnapshot, node: NodeId) -> ConditionBehavior {
    ConditionBehavior {
        edge: EdgePolicy::Level,
        toggle: child_bool(snapshot, node, "toggle_mode").unwrap_or(false),
        transient_ticks: 0,
        validation_delay_ms: seconds_to_millis(
            child_number(snapshot, node, "validation_delay_s").unwrap_or(0.0),
        ),
        invalidation_delay_ms: seconds_to_millis(
            child_number(snapshot, node, "invalidation_delay_s").unwrap_or(0.0),
        ),
    }
}

fn seconds_to_millis(seconds: f64) -> u64 {
    if seconds.is_finite() && seconds > 0.0 {
        (seconds * 1_000.0).round().min(u64::MAX as f64) as u64
    } else {
        0
    }
}

fn comparison_plan(
    snapshot: &ProcessTreeSnapshot,
    condition: NodeId,
    authored_projection: Option<&str>,
    comparator: &str,
) -> (ConditionProjection, TypedComparator) {
    let authored_projection = match authored_projection.unwrap_or_default() {
        "vec2.x" | "x" => ConditionProjection::Vec2X,
        "vec2.y" | "y" => ConditionProjection::Vec2Y,
        "vec3.x" => ConditionProjection::Vec3X,
        "vec3.y" => ConditionProjection::Vec3Y,
        "vec3.z" | "z" => ConditionProjection::Vec3Z,
        "color.r" | "r" => ConditionProjection::ColorRed,
        "color.g" | "g" => ConditionProjection::ColorGreen,
        "color.b" | "b" => ConditionProjection::ColorBlue,
        "color.a" | "a" => ConditionProjection::ColorAlpha,
        _ => ConditionProjection::Identity,
    };
    match comparator {
        "not_equal" => (authored_projection, TypedComparator::NotEqual),
        "is_true" | "is_false" => (authored_projection, TypedComparator::Equal),
        "greater_than" => (authored_projection, TypedComparator::Greater),
        "greater_than_or_equal" => (authored_projection, TypedComparator::GreaterOrEqual),
        "less_than" => (authored_projection, TypedComparator::Less),
        "less_than_or_equal" => (authored_projection, TypedComparator::LessOrEqual),
        "between" => (authored_projection, TypedComparator::Between),
        "outside" => (authored_projection, TypedComparator::Outside),
        "contains" => (authored_projection, TypedComparator::Contains),
        "does_not_contain" => (authored_projection, TypedComparator::DoesNotContain),
        "starts_with" => (authored_projection, TypedComparator::StartsWith),
        "ends_with" => (authored_projection, TypedComparator::EndsWith),
        "regex_match" => (authored_projection, TypedComparator::RegexMatch),
        "value_changed" if source_is_trigger(snapshot, condition) => {
            (authored_projection, TypedComparator::Triggered)
        }
        "value_changed" => (authored_projection, TypedComparator::Changed),
        "magnitude_greater_than" => (magnitude_projection(snapshot, condition), TypedComparator::Greater),
        "magnitude_less_than" => (magnitude_projection(snapshot, condition), TypedComparator::Less),
        "speed_greater_than" => (ConditionProjection::Speed, TypedComparator::Greater),
        "speed_less_than" => (ConditionProjection::Speed, TypedComparator::Less),
        "abs_speed_greater_than" => (ConditionProjection::AbsoluteSpeed, TypedComparator::Greater),
        "abs_speed_less_than" => (ConditionProjection::AbsoluteSpeed, TypedComparator::Less),
        "luminance_greater_than" => (ConditionProjection::ColorLuminance, TypedComparator::Greater),
        "luminance_less_than" => (ConditionProjection::ColorLuminance, TypedComparator::Less),
        "alpha_greater_than" => (ConditionProjection::ColorAlpha, TypedComparator::Greater),
        "alpha_less_than" => (ConditionProjection::ColorAlpha, TypedComparator::Less),
        _ => (authored_projection, TypedComparator::Equal),
    }
}

fn magnitude_projection(
    snapshot: &ProcessTreeSnapshot,
    condition: NodeId,
) -> ConditionProjection {
    let is_vec2 = condition_value_target(snapshot, condition)
        .and_then(|source| snapshot.node(source))
        .and_then(|source| source.param_value.as_ref())
        .is_some_and(|value| matches!(value, ParamValue::Vec2(_, _)));
    if is_vec2 {
        ConditionProjection::Vec2Magnitude
    } else {
        ConditionProjection::Vec3Magnitude
    }
}

fn expected_field(comparator: &str) -> &'static str {
    match comparator {
        "contains" | "does_not_contain" | "starts_with" | "ends_with" | "regex_match" => {
            "reference_string"
        }
        "is_true" => "true",
        "is_false" => "false",
        "equal" | "not_equal" => "reference_auto",
        _ => "reference",
    }
}

fn comparator_is_transient(snapshot: &ProcessTreeSnapshot, condition: NodeId, comparator: &str) -> bool {
    if comparator == "value_changed" {
        return true;
    }
    condition_value_target(snapshot, condition)
        .and_then(|source| snapshot.node(source))
        .is_some_and(|source| matches!(source.param_value, Some(ParamValue::Trigger())))
}

fn source_is_trigger(snapshot: &ProcessTreeSnapshot, condition: NodeId) -> bool {
    condition_value_target(snapshot, condition)
        .and_then(|source| snapshot.node(source))
        .is_some_and(|source| matches!(source.param_value, Some(ParamValue::Trigger())))
}

fn condition_source_target(
    snapshot: &ProcessTreeSnapshot,
    condition: NodeId,
) -> Option<NodeId> {
    child_reference_target(snapshot, condition, "source")
        .or_else(|| child_reference_target(snapshot, condition, "provider_node"))
}

fn condition_value_target(snapshot: &ProcessTreeSnapshot, condition: NodeId) -> Option<NodeId> {
    let target = condition_source_target(snapshot, condition)?;
    let endpoint = child_string(snapshot, condition, "endpoint_id").unwrap_or_default();
    if endpoint.is_empty() {
        Some(target)
    } else {
        snapshot.find_child_by_decl_id(target, &endpoint)
    }
}

fn bind_source(
    snapshot: &ProcessTreeSnapshot,
    condition: NodeId,
    field: &str,
    condition_uuid: uuid::Uuid,
    bindings: &mut HashMap<StableRef, ConditionBinding>,
) -> Option<StableRef> {
    let param = child_param_id(snapshot, condition, field)?;
    let reference = condition_ref(CONDITION_SOURCE_REF_TYPE, condition_uuid, field);
    bindings.insert(reference.clone(), ConditionBinding::Source(param));
    Some(reference)
}

fn bind_param(
    snapshot: &ProcessTreeSnapshot,
    condition: NodeId,
    field: &str,
    condition_uuid: uuid::Uuid,
    bindings: &mut HashMap<StableRef, ConditionBinding>,
) -> Option<StableRef> {
    let param = child_param_id(snapshot, condition, field)?;
    let reference = condition_ref(CONDITION_OPERAND_REF_TYPE, condition_uuid, field);
    bindings.insert(reference.clone(), ConditionBinding::Param(param));
    Some(reference)
}

fn bind_expected(
    snapshot: &ProcessTreeSnapshot,
    condition: NodeId,
    comparator: &str,
    condition_uuid: uuid::Uuid,
    bindings: &mut HashMap<StableRef, ConditionBinding>,
) -> Option<ConditionOperand> {
    match expected_field(comparator) {
        "true" => {
            let reference = condition_ref(CONDITION_OPERAND_REF_TYPE, condition_uuid, "true");
            bindings.insert(reference.clone(), ConditionBinding::Constant(Value::Bool(true)));
            Some(ConditionOperand::Input(reference))
        }
        "false" => {
            let reference = condition_ref(CONDITION_OPERAND_REF_TYPE, condition_uuid, "false");
            bindings.insert(reference.clone(), ConditionBinding::Constant(Value::Bool(false)));
            Some(ConditionOperand::Input(reference))
        }
        "reference_auto" => bind_param(
            snapshot,
            condition,
            automatic_reference_field(snapshot, condition),
            condition_uuid,
            bindings,
        )
        .map(ConditionOperand::Input),
        field => bind_param(
            snapshot,
            condition,
            field,
            condition_uuid,
            bindings,
        )
        .map(ConditionOperand::Input),
    }
}

fn automatic_reference_field(snapshot: &ProcessTreeSnapshot, condition: NodeId) -> &'static str {
    let source = condition_value_target(snapshot, condition)
        .and_then(|source| snapshot.node(source))
        .and_then(|source| source.param_value.as_ref());
    match source {
        Some(ParamValue::Bool(_)) => "reference_bool",
        Some(ParamValue::Str(_) | ParamValue::File(_) | ParamValue::Enum(_)) => {
            "reference_string"
        }
        Some(ParamValue::Vec2(_, _)) => "reference_vec2",
        Some(ParamValue::Vec3(_, _, _)) => "reference_vec3",
        Some(ParamValue::Color(_, _, _, _)) => "reference_color",
        _ => "reference",
    }
}

fn condition_ref(value_type: &str, condition: uuid::Uuid, field: &str) -> StableRef {
    StableRef::new(
        ValueTypeId::new(value_type),
        format!("{condition}/{field}"),
    )
}

fn child_reference_target(
    snapshot: &ProcessTreeSnapshot,
    parent: NodeId,
    decl_id: &str,
) -> Option<NodeId> {
    let ParamValue::Reference(reference) = child_param(snapshot, parent, decl_id)? else {
        return None;
    };
    snapshot.node_id_by_uuid(reference.uuid())
}

fn child_param<'a>(
    snapshot: &'a ProcessTreeSnapshot,
    parent: NodeId,
    decl_id: &str,
) -> Option<&'a ParamValue> {
    snapshot.child_ids(parent).into_iter().find_map(|child| {
        let node = snapshot.node(child)?;
        (node.decl_id == decl_id).then_some(node.param_value.as_ref()).flatten()
    })
}

fn child_param_id(
    snapshot: &ProcessTreeSnapshot,
    parent: NodeId,
    decl_id: &str,
) -> Option<NodeId> {
    snapshot.child_ids(parent).into_iter().find(|child| {
        snapshot
            .node(*child)
            .is_some_and(|node| node.decl_id == decl_id && node.param_value.is_some())
    })
}

fn child_string(snapshot: &ProcessTreeSnapshot, parent: NodeId, decl_id: &str) -> Option<String> {
    match child_param(snapshot, parent, decl_id)? {
        ParamValue::Str(value) | ParamValue::File(value) | ParamValue::Enum(value) => {
            Some(value.clone())
        }
        _ => None,
    }
}

fn child_number(snapshot: &ProcessTreeSnapshot, parent: NodeId, decl_id: &str) -> Option<f64> {
    match child_param(snapshot, parent, decl_id)? {
        ParamValue::Int(value) => Some(f64::from(*value)),
        ParamValue::Float(value) => Some(*value),
        _ => None,
    }
}

fn child_bool(snapshot: &ProcessTreeSnapshot, parent: NodeId, decl_id: &str) -> Option<bool> {
    match child_param(snapshot, parent, decl_id)? {
        ParamValue::Bool(value) => Some(*value),
        _ => None,
    }
}

pub(crate) fn param_to_condition_value(value: &ParamValue) -> Option<Value> {
    match value {
        ParamValue::Trigger() => Some(Value::Trigger(Default::default())),
        ParamValue::Int(value) => Some(Value::Int(i64::from(*value))),
        ParamValue::Float(value) => Some(Value::Float(*value)),
        ParamValue::Str(value) | ParamValue::File(value) | ParamValue::Enum(value) => {
            Some(Value::String(Arc::from(value.as_str())))
        }
        ParamValue::Bool(value) => Some(Value::Bool(*value)),
        ParamValue::Vec2(x, y) => Some(Value::Vec2([*x, *y])),
        ParamValue::Vec3(x, y, z) => Some(Value::Vec3([*x, *y, *z])),
        ParamValue::Color(red, green, blue, alpha) => Some(Value::Color(
            golden_values::ColorValue {
                red: *red,
                green: *green,
                blue: *blue,
                alpha: *alpha,
            },
        )),
        ParamValue::CssValue(_) | ParamValue::Reference(_) => None,
    }
}

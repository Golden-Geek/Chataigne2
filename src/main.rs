use std::io::Error;

use crate::nodes_module_demo::MODULE_MANAGER_UUID;
use golden_core::{
    app::{run_app_with_project_codec, ProjectCodec},
    node::{
        AnimationCurveEasingNode, AnimationCurveKeyNode, AnimationCurveNode, AnimationCurveRangeNode, DashboardGenericWidgetNode, DashboardNode, DashboardNodeWidgetNode, DashboardPageNode, DashboardWidgetContainerNode, Folder, Node, NodeMeta, ParameterAnimationControlNode, UserContextNode,
        DASHBOARD_GENERIC_WIDGET_NODE_TYPE, DASHBOARD_NODE_TYPE, DASHBOARD_NODE_WIDGET_NODE_TYPE, DASHBOARD_PAGE_NODE_TYPE, DASHBOARD_WIDGET_CONTAINER_NODE_TYPE, FOLDER_NODE_TYPE, PARAMETER_ANIMATION_CONTROL_NODE_TYPE, PARAMETER_ANIMATION_CURVE_NODE_TYPE, PARAMETER_ANIMATION_EASING_NODE_TYPE,
        PARAMETER_ANIMATION_KEY_NODE_TYPE, PARAMETER_ANIMATION_RANGE_NODE_TYPE, PARAMETER_NODE_TYPES, USER_CONTEXT_NODE_TYPE,
    },
    parameter::{ParamValue, Parameter, ParameterChangeCheck, ParameterConstraints, ParameterControlState, ParameterEventBehaviour, ParameterUiHints},
    script::{ScriptBudgets, ScriptNode, ScriptNodeConfig},
};
use serde::{Deserialize, Serialize};

include!(concat!(env!("OUT_DIR"), "/app_nodes.rs"));
pub type AppEngine = golden_core::engine::Engine<AppNode>;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ParameterProjectData {
    value: ParamValue,
    default_value: ParamValue,
    change_check: ParameterChangeCheck,
    event_behaviour: ParameterEventBehaviour,
    read_only: bool,
    constraints: ParameterConstraints,
    ui_hints: ParameterUiHints,
    control: ParameterControlState,
    #[serde(default = "default_true")]
    control_modes_enabled: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct RawParameterProjectData {
    value: serde_json::Value,
    default_value: serde_json::Value,
    change_check: ParameterChangeCheck,
    event_behaviour: ParameterEventBehaviour,
    read_only: bool,
    constraints: ParameterConstraints,
    ui_hints: ParameterUiHints,
    control: ParameterControlState,
    #[serde(default = "default_true")]
    control_modes_enabled: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct RawLegacyParameterProjectData {
    value: serde_json::Value,
    change_check: ParameterChangeCheck,
    event_behaviour: ParameterEventBehaviour,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ScriptProjectData {
    config: ScriptNodeConfig,
    #[serde(default)]
    budgets: ScriptBudgets,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ModuleManagerProjectData {
    #[serde(default = "default_true")]
    allow_dmx: bool,
}

fn default_true() -> bool {
    true
}

fn default_parameter_value_for_node_type(node_type: &str) -> Option<ParamValue> {
    match node_type {
        "trigger" => Some(ParamValue::Trigger()),
        "int" => Some(ParamValue::Int(0)),
        "float" => Some(ParamValue::Float(0.0)),
        "str" => Some(ParamValue::Str(String::new())),
        "file" => Some(ParamValue::File(String::new())),
        "enum" => Some(ParamValue::Enum(String::new())),
        "bool" => Some(ParamValue::Bool(false)),
        "vec2" => Some(ParamValue::Vec2(0.0, 0.0)),
        "vec3" => Some(ParamValue::Vec3(0.0, 0.0, 0.0)),
        "color" => Some(ParamValue::Color(1.0, 1.0, 1.0, 1.0)),
        "reference" => Some(ParamValue::Reference(golden_core::node::NodeReference::default())),
        _ => None,
    }
}

fn decode_project_param_value(value: &serde_json::Value) -> Result<ParamValue, String> {
    if value.is_null() {
        return Ok(ParamValue::Trigger());
    }

    if let Ok(decoded) = serde_json::from_value::<ParamValue>(value.clone()) {
        return Ok(decoded);
    }

    if value.as_object().is_some_and(|object| object.len() == 1 && object.contains_key("Trigger")) {
        return Ok(ParamValue::Trigger());
    }

    ParamValue::from_script_json(value)
}

fn encode_project_node(node: &AppNode) -> Result<serde_json::Value, String> {
    let encoded = match node {
        AppNode::ModuleManager(module_manager) => serde_json::to_value(ModuleManagerProjectData { allow_dmx: module_manager.allow_dmx() }),
        AppNode::Parameter(parameter) => serde_json::to_value(ParameterProjectData {
            value: parameter.value.clone(),
            default_value: parameter.default_value.clone(),
            change_check: parameter.change_check.clone(),
            event_behaviour: parameter.event_behaviour,
            read_only: parameter.read_only,
            constraints: parameter.constraints.clone(),
            ui_hints: parameter.ui_hints.clone(),
            control: parameter.control.clone(),
            control_modes_enabled: parameter.control_modes_enabled,
        }),
        AppNode::Script(script) => serde_json::to_value(ScriptProjectData { config: script.config.clone(), budgets: script.budgets }),
        _ => Ok(serde_json::Value::Null),
    };

    encoded.map_err(|err| format!("failed to encode node data: {err}"))
}

fn decode_parameter_node_data(node_type: &str, data: &serde_json::Value, meta: &NodeMeta) -> Result<Parameter, String> {
    let parsed = if data.is_null() {
        let fallback_value = default_parameter_value_for_node_type(node_type).ok_or_else(|| format!("unsupported parameter node type '{node_type}'"))?;
        ParameterProjectData {
            value: fallback_value.clone(),
            default_value: fallback_value,
            change_check: ParameterChangeCheck::ValueChange,
            event_behaviour: ParameterEventBehaviour::Coalesce,
            read_only: false,
            constraints: ParameterConstraints::default(),
            ui_hints: ParameterUiHints::default(),
            control: ParameterControlState::default(),
            control_modes_enabled: true,
        }
    } else if let Ok(full) = serde_json::from_value::<RawParameterProjectData>(data.clone()) {
        ParameterProjectData {
            value: decode_project_param_value(&full.value).map_err(|err| format!("invalid parameter payload: {err}"))?,
            default_value: decode_project_param_value(&full.default_value).map_err(|err| format!("invalid parameter payload: {err}"))?,
            change_check: full.change_check,
            event_behaviour: full.event_behaviour,
            read_only: full.read_only,
            constraints: full.constraints,
            ui_hints: full.ui_hints,
            control: full.control,
            control_modes_enabled: full.control_modes_enabled,
        }
    } else {
        let legacy = serde_json::from_value::<RawLegacyParameterProjectData>(data.clone()).map_err(|err| format!("invalid parameter payload: {err}"))?;
        ParameterProjectData {
            value: decode_project_param_value(&legacy.value).map_err(|err| format!("invalid parameter payload: {err}"))?,
            default_value: decode_project_param_value(&legacy.value).map_err(|err| format!("invalid parameter payload: {err}"))?,
            change_check: legacy.change_check,
            event_behaviour: legacy.event_behaviour,
            read_only: false,
            constraints: ParameterConstraints::default(),
            ui_hints: ParameterUiHints::default(),
            control: ParameterControlState::default(),
            control_modes_enabled: true,
        }
    };

    let mut node = Parameter::new(&meta.label, parsed.value.clone(), parsed.change_check);
    node.value = parsed.value;
    node.default_value = parsed.default_value;
    node.event_behaviour = parsed.event_behaviour;
    node.read_only = parsed.read_only;
    node.constraints = parsed.constraints;
    node.ui_hints = parsed.ui_hints;
    node.control = parsed.control;
    node.control_modes_enabled = parsed.control_modes_enabled;
    Ok(node)
}

fn decode_project_node(node_type: &str, data: &serde_json::Value, meta: &NodeMeta) -> Result<AppNode, String> {
    if PARAMETER_NODE_TYPES.contains(&node_type) {
        return decode_parameter_node_data(node_type, data, meta).map(Into::into);
    }

    match node_type {
        FOLDER_NODE_TYPE => Ok(Folder::new(meta.label.clone()).into()),
        USER_CONTEXT_NODE_TYPE => Ok(UserContextNode::new(meta.label.clone()).into()),
        DASHBOARD_NODE_TYPE => Ok(DashboardNode::new(meta.label.clone()).into()),
        DASHBOARD_PAGE_NODE_TYPE => Ok(DashboardPageNode::new(meta.label.clone()).into()),
        DASHBOARD_WIDGET_CONTAINER_NODE_TYPE => Ok(DashboardWidgetContainerNode::new(meta.label.clone()).into()),
        DASHBOARD_NODE_WIDGET_NODE_TYPE => Ok(DashboardNodeWidgetNode::new(meta.label.clone()).into()),
        DASHBOARD_GENERIC_WIDGET_NODE_TYPE => Ok(DashboardGenericWidgetNode::new(meta.label.clone()).into()),
        PARAMETER_ANIMATION_CONTROL_NODE_TYPE => Ok(ParameterAnimationControlNode::new(meta.label.clone()).into()),
        PARAMETER_ANIMATION_CURVE_NODE_TYPE => Ok(AnimationCurveNode::new_with_label(meta.label.clone()).into()),
        PARAMETER_ANIMATION_RANGE_NODE_TYPE => Ok(AnimationCurveRangeNode::new(None, true).into()),
        PARAMETER_ANIMATION_KEY_NODE_TYPE => Ok(AnimationCurveKeyNode::new_with_label(meta.label.clone()).into()),
        PARAMETER_ANIMATION_EASING_NODE_TYPE => Ok(AnimationCurveEasingNode::new(meta.label.clone()).into()),
        "script" => {
            let parsed = if data.is_null() {
                ScriptProjectData {
                    config: ScriptNodeConfig::default(),
                    budgets: ScriptBudgets::default(),
                }
            } else {
                serde_json::from_value::<ScriptProjectData>(data.clone()).map_err(|err| format!("invalid script payload: {err}"))?
            };
            let mut node = ScriptNode::new(meta.label.clone(), parsed.config);
            node.budgets = parsed.budgets;
            Ok(node.into())
        }
        "module_manager" => {
            let parsed = if data.is_null() {
                ModuleManagerProjectData { allow_dmx: true }
            } else {
                serde_json::from_value::<ModuleManagerProjectData>(data.clone()).map_err(|err| format!("invalid module_manager payload: {err}"))?
            };
            Ok(ModuleManager::create(meta.label.clone(), parsed.allow_dmx).into())
        }
        "module_base" => Ok(ModuleBase::new(meta.label.clone()).into()),
        "osc_module" => Ok(OscModule::create(meta.label.clone()).into()),
        "midi_module" => Ok(MidiModule::create(meta.label.clone()).into()),
        "dmx_module" => Ok(DmxModule::create(meta.label.clone()).into()),
        _ => Err(format!("unsupported node type '{node_type}'")),
    }
}

fn app_project_codec() -> ProjectCodec<AppNode> {
    ProjectCodec::new(encode_project_node, decode_project_node)
}

fn main() -> std::io::Result<()> {
    let root: AppNode = Folder::new("Root".to_string()).into();
    let mut engine = AppEngine::new(root);
    engine.register_reference_filter("module_values_parameters", |engine, _param_node, _root, candidate| {
        let Some(candidate_node) = engine.nodes.get(candidate) else {
            return false;
        };
        if candidate_node.engine_param_snapshot().is_none() {
            return false;
        }

        let Some(parent_id) = candidate_node.node_data().parent else {
            return false;
        };
        let Some(parent_node) = engine.nodes.get(parent_id) else {
            return false;
        };
        if parent_node.node_data().meta.decl_id.0 != "values" {
            return false;
        }

        let mut current = Some(parent_id);
        let mut has_module_ancestor = false;
        let mut has_module_manager_ancestor = false;
        while let Some(node_id) = current {
            let Some(node) = engine.nodes.get(node_id) else {
                break;
            };
            if node.user_item_kind() == "module" {
                has_module_ancestor = true;
            }
            if node.get_type() == "module_manager" {
                has_module_manager_ancestor = true;
            }
            current = node.node_data().parent;
        }

        has_module_ancestor && has_module_manager_ancestor
    });

    let mut manager_node = ModuleManager::create("Module Manager", true);
    manager_node.node_data_mut().meta.uuid = MODULE_MANAGER_UUID;
    engine.add_node(manager_node.into(), None);
    engine.apply_edits().map_err(|err| Error::other(format!("failed to create module manager: {err}")))?;

    let manager = engine.nodes.get(engine.root).and_then(|root| root.node_data().first_child).ok_or_else(|| Error::other("module manager node was not attached under root"))?;

    engine.add_node(DashboardNode::new("Dashboard").into(), None);
    engine.apply_edits().map_err(|err| Error::other(format!("failed to create dashboard root: {err}")))?;

    engine.add_node(Folder::new("Module Folder").into(), Some(manager));
    engine.apply_edits().map_err(|err| Error::other(format!("failed to create module folder: {err}")))?;

    let module_folder = engine.nodes.get(manager).and_then(|manager_node| manager_node.node_data().first_child).ok_or_else(|| Error::other("module folder node was not attached under module manager"))?;

    engine.add_user_item(OscModule::create("OSC Module").into(), Some(module_folder));
    engine.add_user_item(MidiModule::create("MIDI Module").into(), Some(manager));
    engine.add_user_item(DmxModule::create("DMX Module").into(), Some(module_folder));

    run_app_with_project_codec(engine, app_project_codec())
}

#[cfg(test)]
mod tests {
    use super::*;
    use golden_core::node::{DeclId, NodeMeta, NodeUserPermissions, NodeUuid, PresentationHint, SemanticsHint};

    fn test_meta(label: &str) -> NodeMeta {
        NodeMeta {
            uuid: NodeUuid::nil(),
            decl_id: DeclId(label.to_string()),
            short_name: label.to_string(),
            enabled: true,
            can_be_disabled: true,
            label: label.to_string(),
            description: None,
            declared_description_key: None,
            declared_description: None,
            tags: Vec::new(),
            user_permissions: NodeUserPermissions::default(),
            semantics: SemanticsHint::default(),
            presentation: PresentationHint::default(),
        }
    }

    #[test]
    fn decode_trigger_parameter_payload_accepts_encoded_null_values() {
        let meta = test_meta("Trigger");
        let payload = encode_project_node(&AppNode::Parameter(Parameter::new("Trigger", ParamValue::Trigger(), ParameterChangeCheck::ValueChange).into())).expect("trigger parameter payload should encode");

        let node = decode_parameter_node_data("trigger", &payload, &meta).expect("trigger parameter payload should decode");
        assert!(matches!(node.value, ParamValue::Trigger()));
        assert!(matches!(node.default_value, ParamValue::Trigger()));
    }
}

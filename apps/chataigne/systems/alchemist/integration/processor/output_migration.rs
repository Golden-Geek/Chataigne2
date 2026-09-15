//! One-time migration from Mapping-owned Output Target ANodes to concrete commands.

use chataigne_alchemist::{StableRef, SurfaceItemKind};
use chataigne_state_machine::{
    alchemist::OUTPUT_TARGET_TYPE, OutputArgumentBinding, OutputBindingConfig,
    OUTPUT_BINDINGS_FIELD, OUTPUT_TARGET_FIELD,
};
use golden_core::{
    edit::{Edit, NodeTree},
    node::{Node, NodeId, NodeMetaPatch, NodeReference, NodeUuid, PresentationHint},
    parameter::{ParamValue, ParameterEventBehaviour},
};
use golden_values::Value as RuntimeValue;

use crate::app::{
    systems_alchemist_formula::{
        anode_from_snapshot, param_to_untyped_runtime_value, ANODE_NODE_TYPE,
    },
    systems_alchemist_generic_commands::{
        GenericInvokeCommand, GenericSetParameterCommand,
    },
    systems_alchemist_managed_nodes::{
        mapping_command_bindings_tree, MappingCommandArgumentBinding,
        MappingCommandBindings, UNRESOLVED_LEGACY_BINDINGS_DECL,
    },
    AppEngine,
};

use super::managed_region_roles_from_tags;

pub(crate) const MAPPING_CONCRETE_OUTPUTS_V1_TAG: &str =
    "chataigne.mapping.concrete_outputs.v1";
const LEGACY_TARGET_DECL_ID: &str = "unresolved_legacy_target";
const OUTPUT_TARGET_TYPE_TAG: &str = "alchemist.anode.type:chataigne.output_target";

#[derive(Clone)]
struct MigrationRecord {
    adapter: NodeId,
    uuid: NodeUuid,
    parent: NodeId,
    previous_uuid: Option<NodeUuid>,
    label: String,
    enabled: bool,
    can_be_disabled: bool,
    presentation: PresentationHint,
    target: StableRef,
    bindings: OutputBindingConfig,
    unresolved_bindings: Option<String>,
    replacement: ReplacementKind,
}

#[derive(Clone, Copy)]
enum ReplacementKind {
    SetParameter,
    Invoke,
}

pub(crate) fn migrate_mapping_output_adapters(
    engine: &mut AppEngine,
) -> Result<(), String> {
    let snapshot = engine.process_tree_snapshot();
    let root = snapshot.root();
    let root_node = snapshot.node(root).ok_or("project root is missing")?;
    if root_node
        .tags
        .iter()
        .any(|tag| tag == MAPPING_CONCRETE_OUTPUTS_V1_TAG)
    {
        return Ok(());
    }

    let mut records = Vec::new();
    for (adapter, node) in engine.nodes.iter() {
        if node.get_type() != ANODE_NODE_TYPE
            || !node
                .node_data()
                .meta
                .tags
                .iter()
                .any(|tag| tag == OUTPUT_TARGET_TYPE_TAG)
        {
            continue;
        }
        let Some(parent) = snapshot.node(adapter).and_then(|node| node.parent) else {
            continue;
        };
        if !snapshot.node(parent).is_some_and(|parent| {
            managed_region_roles_from_tags(&parent.tags).contains(&SurfaceItemKind::Output)
        }) {
            continue;
        }
        let anode = anode_from_snapshot(&snapshot, adapter)
            .map_err(|error| format!("Mapping output {adapter:?} cannot be migrated: {error}"))?;
        if anode.type_id.as_str() != OUTPUT_TARGET_TYPE {
            continue;
        }
        let target = match anode.config.get(OUTPUT_TARGET_FIELD) {
            Some(RuntimeValue::Ref(target)) => target.clone(),
            _ => {
                return Err(format!(
                    "Mapping output {adapter:?} has no stable target reference"
                ));
            }
        };
        let bindings = anode
            .config
            .get(OUTPUT_BINDINGS_FIELD)
            .map(OutputBindingConfig::from_runtime_value)
            .transpose()
            .map_err(|error| {
                format!("Mapping output {adapter:?} has invalid bindings: {error}")
            })?
            .unwrap_or_default();
        let replacement = target
            .stable_id
            .parse()
            .ok()
            .and_then(|uuid| snapshot.node_id_by_uuid(golden_core::node::NodeUuid(uuid)))
            .filter(|target| {
                snapshot
                    .node(*target)
                    .is_some_and(|node| node.param_value.is_some())
            })
            .map_or(ReplacementKind::Invoke, |_| ReplacementKind::SetParameter);
        let unresolved_bindings = (!bindings_have_ordinary_controls(&bindings))
            .then(|| bindings.to_authoring_json())
            .transpose()
            .map_err(|error| {
                format!("Mapping output {adapter:?} bindings cannot be retained: {error}")
            })?;
        records.push(MigrationRecord {
            adapter,
            uuid: node.node_data().meta.uuid,
            parent,
            previous_uuid: previous_sibling(&snapshot, parent, adapter)
                .and_then(|previous| snapshot.node(previous))
                .map(|previous| previous.uuid),
            label: snapshot.node(adapter).unwrap().label.clone(),
            enabled: snapshot.node(adapter).unwrap().enabled,
            can_be_disabled: snapshot.node(adapter).unwrap().can_be_disabled,
            presentation: snapshot.node(adapter).unwrap().presentation.clone(),
            target,
            bindings,
            unresolved_bindings,
            replacement,
        });
    }

    drop(snapshot);
    for record in &mut records {
        let mut replacement: Box<dyn Node> = match record.replacement {
            ReplacementKind::SetParameter => Box::new(GenericSetParameterCommand::create()),
            ReplacementKind::Invoke => Box::new(GenericInvokeCommand::create()),
        };
        let meta = &mut replacement.node_data_mut().meta;
        meta.uuid = record.uuid;
        meta.label = record.label.clone();
        meta.enabled = record.enabled;
        meta.can_be_disabled = record.can_be_disabled;
        meta.presentation = record.presentation.clone();
        let previous = record
            .previous_uuid
            .and_then(|uuid| engine.process_tree_snapshot().node_id_by_uuid(uuid));
        engine.edits.push(Edit::RemoveNode {
            node: record.adapter,
        });
        engine.edits.push(Edit::AddUserItemTree {
            parent: record.parent,
            prev_sibling: previous,
            tree: NodeTree::boxed(replacement)
                .with_child(mapping_command_bindings_tree(None))
                .as_user_item(),
        });
        apply_migration_edits(engine, "concrete command replacement")?;
        record.adapter = engine
            .process_tree_snapshot()
            .node_id_by_uuid(record.uuid)
            .ok_or_else(|| format!("migrated command {} was not materialized", record.uuid.0))?;
    }

    let snapshot = engine.process_tree_snapshot();
    for record in &records {
        let target_param = snapshot
            .find_child_by_decl_id(record.adapter, "target")
            .ok_or_else(|| format!("migrated command {:?} has no target", record.adapter))?;
        let target_uuid = record.target.stable_id.parse::<uuid::Uuid>().ok();
        if let Some(target_uuid) = target_uuid {
            engine.edits.push(Edit::SetParam {
                node: target_param,
                value: ParamValue::Reference(NodeReference::new(
                    golden_core::node::NodeUuid(target_uuid),
                )),
                behaviour: ParameterEventBehaviour::Coalesce,
            });
        }

        let bindings = match (record.unresolved_bindings.is_some(), record.replacement) {
            (true, _) => OutputBindingConfig::default(),
            (false, ReplacementKind::Invoke) => record.bindings.clone(),
            (false, ReplacementKind::SetParameter) => {
                let value = snapshot
                    .find_child_by_decl_id(record.adapter, "value")
                    .and_then(|parameter| stable_parameter_ref(&snapshot, parameter))
                    .ok_or_else(|| {
                        format!("migrated Set Parameter {:?} has no value", record.adapter)
                    })?;
                OutputBindingConfig {
                    value: record.bindings.value.clone(),
                    arguments: vec![OutputArgumentBinding {
                        parameter: value,
                        source: record.bindings.value.clone(),
                    }],
                    send_policy: record.bindings.send_policy,
                }
            }
        };
        let bindings_node = snapshot
            .find_child_by_decl_id(record.adapter, "mapping_bindings")
            .ok_or_else(|| {
                format!("migrated command {:?} has no Mapping binding container", record.adapter)
            })?;
        engine.edits.push(Edit::CallNodeMutation {
            node: bindings_node,
            callback: Box::new(move |node, ctx| {
                let Some(node) = node.as_any_mut().downcast_mut::<MappingCommandBindings>() else {
                    return Err("expected Mapping command bindings".to_owned());
                };
                node.apply_config(ctx, bindings);
                Ok(())
            }),
            needs_tree_snapshot: true,
        });

        if let Some(unresolved) = &record.unresolved_bindings {
            let unresolved_param = snapshot
                .find_child_by_decl_id(bindings_node, UNRESOLVED_LEGACY_BINDINGS_DECL)
                .ok_or_else(|| {
                    format!(
                        "migrated command {:?} has no legacy binding resolution field",
                        record.adapter
                    )
                })?;
            engine.edits.push(Edit::SetParam {
                node: unresolved_param,
                value: ParamValue::Str(unresolved.clone()),
                behaviour: ParameterEventBehaviour::Coalesce,
            });
        }

        if target_uuid.is_none() {
            let legacy_target = snapshot
                .find_child_by_decl_id(record.adapter, LEGACY_TARGET_DECL_ID)
                .ok_or_else(|| {
                    format!("migrated command {:?} has no resolution field", record.adapter)
                })?;
            engine.edits.push(Edit::SetParam {
                node: legacy_target,
                value: ParamValue::Str(format!(
                    "{}:{}",
                    record.target.value_type, record.target.stable_id
                )),
                behaviour: ParameterEventBehaviour::Coalesce,
            });
        }
    }
    drop(snapshot);
    if !records.is_empty() {
        apply_migration_edits(engine, "command target and binding materialization")?;
        apply_migration_edits(engine, "binding child insertion")?;
    }

    let snapshot = engine.process_tree_snapshot();
    let argument_nodes = records
        .iter()
        .filter_map(|record| {
            snapshot.find_child_by_decl_id(record.adapter, "mapping_bindings")
        })
        .flat_map(|bindings| snapshot.child_ids(bindings))
        .filter(|binding| {
            snapshot.node(*binding).is_some_and(|node| {
                node.node_type == "mapping_command_argument_binding"
            })
        })
        .collect::<Vec<_>>();
    drop(snapshot);
    for argument in argument_nodes {
        engine.edits.push(Edit::CallNodeMutation {
            node: argument,
            callback: Box::new(|node, ctx| {
                let Some(node) = node
                    .as_any_mut()
                    .downcast_mut::<MappingCommandArgumentBinding>()
                else {
                    return Err("expected Mapping command argument binding".to_owned());
                };
                node.apply_pending(ctx);
                Ok(())
            }),
            needs_tree_snapshot: true,
        });
    }
    if !records.is_empty() {
        apply_migration_edits(engine, "argument binding materialization")?;
    }

    let snapshot = engine.process_tree_snapshot();
    let mut tags = snapshot
        .node(root)
        .ok_or("project root is missing after Mapping output migration")?
        .tags
        .clone();
    tags.push(MAPPING_CONCRETE_OUTPUTS_V1_TAG.to_owned());
    engine.edits.push(Edit::PatchMeta {
        node: root,
        patch: NodeMetaPatch {
            tags: Some(tags),
            ..NodeMetaPatch::default()
        },
    });
    for record in records.iter().filter(|record| {
        record.target.stable_id.parse::<uuid::Uuid>().is_err()
    }) {
        engine.set_node_warning(
            record.adapter,
            "Legacy Mapping target could not be resolved. Select Command to finish migration.",
        );
    }
    for record in records
        .iter()
        .filter(|record| record.unresolved_bindings.is_some())
    {
        engine.set_node_warning(
            record.adapter,
            "Legacy Mapping bindings contain a constant that ordinary controls cannot represent. The original binding document is retained; configure the Mapping Binding controls to resolve it.",
        );
    }
    drop(snapshot);
    apply_migration_edits(engine, "migration marker")
}

fn bindings_have_ordinary_controls(bindings: &OutputBindingConfig) -> bool {
    source_has_ordinary_controls(&bindings.value)
        && bindings
            .arguments
            .iter()
            .all(|binding| source_has_ordinary_controls(&binding.source))
}

fn source_has_ordinary_controls(source: &chataigne_state_machine::OutputValueSource) -> bool {
    use chataigne_state_machine::OutputValueSource;

    match source {
        OutputValueSource::Whole
        | OutputValueSource::Element(_)
        | OutputValueSource::Component { .. } => true,
        OutputValueSource::Constant(value) => matches!(
            value,
            RuntimeValue::Bool(_)
                | RuntimeValue::Float(_)
                | RuntimeValue::String(_)
                | RuntimeValue::Vec2(_)
                | RuntimeValue::Vec3(_)
                | RuntimeValue::Color(_)
        ) || matches!(value, RuntimeValue::Int(value) if i32::try_from(*value).is_ok()),
    }
}

fn stable_parameter_ref(
    snapshot: &golden_core::process_ctx::ProcessTreeSnapshot,
    parameter: NodeId,
) -> Option<StableRef> {
    let node = snapshot.node(parameter)?;
    let value_type = param_to_untyped_runtime_value(node.param_value.as_ref()?)
        .ok()?
        .value_type();
    Some(StableRef::new(
        value_type,
        node.uuid.0.to_string(),
    ))
}

fn previous_sibling(
    snapshot: &golden_core::process_ctx::ProcessTreeSnapshot,
    parent: NodeId,
    child: NodeId,
) -> Option<NodeId> {
    snapshot
        .child_ids(parent)
        .into_iter()
        .take_while(|candidate| *candidate != child)
        .last()
}

fn apply_migration_edits(engine: &mut AppEngine, step: &str) -> Result<(), String> {
    engine
        .apply_project_load_edits()
        .map_err(|error| format!("Mapping concrete-output migration {step} failed: {error}"))
}

//! Immutable, node-independent source for explicitly compressed Mappings.
//!
//! R06 prepares and exercises this representation without exposing a product
//! switch. R07 owns the atomic transition that persists it and removes/restores
//! the authored manager subtrees.

use std::collections::{HashMap, HashSet};

use chataigne_alchemist::{
    AlchemistFormula, AlchemistFormulaInstance, ManagedRegionId, RuntimeIntent,
};
use chataigne_state_machine::{
    alchemist::{shared_node_registry, shared_value_type_registry},
    CommandArgumentValues, ManagedFormulaRuntime, OutputBindingConfig,
};
use golden_core::{
    app::capture_sparse_subtree_file,
    engine::{ProjectFile, ProjectPersistenceError},
    node::{Node, NodeId, NodeUuid},
    parameter::{coerce_param_value_for_target, ParamValue, ParameterControlMode},
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};
use serde::{Deserialize, Serialize};

use crate::app::AppEngine;
use crate::app::module_command;
use crate::app::systems_alchemist_formula::{
    formula_from_snapshot, runtime_value_to_param,
};
use crate::app::systems_alchemist_generic_commands::{
    execute_prepared_generic_command, PreparedGenericCommandInvocation,
    GENERIC_SET_PARAMETER_COMMAND_NODE_TYPE,
    GENERIC_TRIGGER_PARAMETER_COMMAND_NODE_TYPE,
};
use crate::app::systems_alchemist_managed_nodes::{
    is_output_node, mapping_output_binding_config,
};

use super::{
    managed_regions_from_snapshot, processor_formula_source_ref,
    processor_managed_region_decl_id, FormulaSourceRef,
};

pub(crate) const FROZEN_MAPPING_SOURCE_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct FrozenMappingSource {
    pub(crate) version: u32,
    pub(crate) formula_source_uuid: NodeUuid,
    pub(crate) formula: AlchemistFormula,
    pub(crate) formula_instance: AlchemistFormulaInstance,
    pub(crate) authored_regions: Vec<FrozenAuthoredRegion>,
    pub(crate) commands: Vec<FrozenCommandDefinition>,
}

impl FrozenMappingSource {
    pub(crate) fn compile_runtime(
        &self,
    ) -> Result<Option<ManagedFormulaRuntime>, String> {
        if self.version != FROZEN_MAPPING_SOURCE_VERSION {
            return Err(format!(
                "unsupported frozen Mapping source version {}; expected {}",
                self.version, FROZEN_MAPPING_SOURCE_VERSION
            ));
        }
        let value_types = shared_value_type_registry();
        let nodes = shared_node_registry();
        let compile = chataigne_alchemist::CompileCtx {
            value_types,
            nodes,
            properties: Some(&self.formula.properties),
        };
        ManagedFormulaRuntime::compile(&self.formula, &self.formula_instance, &compile)
            .map_err(|error| error.to_string())
    }

    pub(crate) fn command_for_intent(
        &self,
        intent: &RuntimeIntent,
    ) -> Result<&FrozenCommandDefinition, String> {
        let target = intent
            .target
            .as_ref()
            .ok_or_else(|| "frozen command intent has no target".to_owned())?;
        let uuid = target
            .stable_id
            .parse::<uuid::Uuid>()
            .map(NodeUuid)
            .map_err(|_| format!("frozen command target '{}' is not a UUID", target.stable_id))?;
        self.commands
            .iter()
            .find(|command| command.uuid == uuid)
            .ok_or_else(|| format!("frozen command target {} is unavailable", uuid.0))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct FrozenAuthoredRegion {
    pub(crate) region_id: ManagedRegionId,
    pub(crate) document: ProjectFile,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct FrozenCommandDefinition {
    pub(crate) uuid: NodeUuid,
    pub(crate) label: String,
    pub(crate) enabled: bool,
    pub(crate) bindings: OutputBindingConfig,
    pub(crate) parameters: Vec<FrozenCommandParameter>,
    pub(crate) operation: FrozenGenericCommandOperation,
}

impl FrozenCommandDefinition {
    pub(crate) fn execute_intent(
        &self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        intent: &RuntimeIntent,
    ) -> Result<(), String> {
        if !self.enabled {
            return Ok(());
        }
        let arguments = CommandArgumentValues::from_runtime_value(&intent.payload)?
            .map_or_else(Vec::new, |bound| bound.arguments);
        let invocation = self.prepare_invocation(arguments.as_slice())?;
        execute_prepared_generic_command(ctx, snapshot, invocation)
    }

    fn prepare_invocation(
        &self,
        arguments: &[chataigne_state_machine::ResolvedCommandArgument],
    ) -> Result<PreparedGenericCommandInvocation, String> {
        let mut overrides = HashMap::new();
        for argument in arguments {
            let uuid = argument
                .parameter
                .stable_id
                .parse::<uuid::Uuid>()
                .map(NodeUuid)
                .map_err(|_| {
                    format!(
                        "frozen command argument '{}' is not a UUID",
                        argument.parameter.stable_id
                    )
                })?;
            let expected = self
                .parameter(uuid)
                .ok_or_else(|| format!("frozen command argument {} is unavailable", uuid.0))?;
            if overrides.contains_key(&uuid) {
                return Err(format!("frozen command argument {} is bound twice", uuid.0));
            }
            let value = runtime_value_to_param(&argument.value)?;
            let value = coerce_param_value_for_target(&value, &expected.value, None)
                .ok_or_else(|| {
                    format!(
                        "frozen command argument {} cannot convert '{}' to the target parameter type",
                        uuid.0,
                        argument.value.value_type()
                    )
                })?;
            overrides.insert(uuid, value);
        }

        let value = |uuid| {
            overrides
                .get(&uuid)
                .cloned()
                .or_else(|| self.parameter(uuid).map(|parameter| parameter.value.clone()))
                .ok_or_else(|| format!("frozen command parameter {} is unavailable", uuid.0))
        };
        match self.operation {
            FrozenGenericCommandOperation::SetParameter {
                target_parameter,
                value_parameter,
            } => Ok(PreparedGenericCommandInvocation::SetParameter {
                target: value(target_parameter)?,
                value: value(value_parameter)?,
            }),
            FrozenGenericCommandOperation::TriggerParameter { target_parameter } => {
                Ok(PreparedGenericCommandInvocation::TriggerParameter {
                    target: value(target_parameter)?,
                })
            }
        }
    }

    fn parameter(&self, uuid: NodeUuid) -> Option<&FrozenCommandParameter> {
        self.parameters.iter().find(|parameter| parameter.uuid == uuid)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct FrozenCommandParameter {
    pub(crate) uuid: NodeUuid,
    pub(crate) decl_id: String,
    pub(crate) value: ParamValue,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum FrozenGenericCommandOperation {
    SetParameter {
        target_parameter: NodeUuid,
        value_parameter: NodeUuid,
    },
    TriggerParameter {
        target_parameter: NodeUuid,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct FrozenMappingBlocker {
    pub(crate) code: String,
    pub(crate) node: Option<NodeUuid>,
    pub(crate) detail: String,
}

impl FrozenMappingBlocker {
    fn new(code: &str, node: Option<NodeUuid>, detail: impl Into<String>) -> Self {
        Self {
            code: code.to_owned(),
            node,
            detail: detail.into(),
        }
    }
}

pub(crate) fn prepare_frozen_mapping_source(
    engine: &AppEngine,
    processor: NodeId,
) -> Result<FrozenMappingSource, Vec<FrozenMappingBlocker>> {
    let snapshot = engine.process_tree_snapshot();
    let source = processor_formula_source_ref(&snapshot, processor)
        .ok_or_else(|| vec![FrozenMappingBlocker::new(
            "missing_formula",
            snapshot.node(processor).map(|node| node.uuid),
            "Mapping has no Formula source",
        )])?;
    let FormulaSourceRef::ProjectNode(source_ref) = source;
    let formula_node = snapshot.node_id_by_uuid(source_ref.uuid()).ok_or_else(|| {
        vec![FrozenMappingBlocker::new(
            "missing_formula",
            Some(source_ref.uuid()),
            "Mapping Formula source is unavailable",
        )]
    })?;
    let formula = formula_from_snapshot(&snapshot, formula_node).map_err(|error| {
        vec![FrozenMappingBlocker::new(
            "invalid_formula",
            Some(source_ref.uuid()),
            error.to_string(),
        )]
    })?;
    let mut formula_instance = formula.instantiate();
    let mut blockers = Vec::new();
    match managed_regions_from_snapshot(&snapshot, processor, &formula) {
        Some(managed_regions) => formula_instance.managed_regions = managed_regions,
        None => blockers.push(FrozenMappingBlocker::new(
            "invalid_authored_regions",
            snapshot.node(processor).map(|node| node.uuid),
            "Mapping managed regions cannot be materialized",
        )),
    }

    let mut authored_regions = Vec::with_capacity(formula.surface.managed_regions.len());
    let mut workflow_nodes = HashSet::new();
    let mut workflow_uuids = HashSet::new();
    let mut commands = Vec::new();
    for definition in &formula.surface.managed_regions {
        let decl_id = processor_managed_region_decl_id(definition.id.as_str());
        let Some(region) = snapshot.find_child_by_decl_id(processor, &decl_id) else {
            blockers.push(FrozenMappingBlocker::new(
                "missing_region",
                snapshot.node(processor).map(|node| node.uuid),
                format!("Mapping region '{}' is missing", definition.label),
            ));
            continue;
        };
        collect_subtree(&snapshot, region, &mut workflow_nodes, &mut workflow_uuids);
        match capture_sparse_subtree_file(engine, region) {
            Ok(document) => authored_regions.push(FrozenAuthoredRegion {
                region_id: definition.id.clone(),
                document,
            }),
            Err(error) => blockers.push(persistence_blocker(&snapshot, region, error)),
        }

        if definition.kind == chataigne_alchemist::ManagedRegionKind::OutputSet {
            for command in snapshot.child_ids(region) {
                match prepare_frozen_command(&snapshot, command) {
                    Ok(command) => commands.push(command),
                    Err(blocker) => blockers.push(blocker),
                }
            }
        }
    }

    collect_inbound_reference_blockers(
        engine,
        &workflow_nodes,
        &workflow_uuids,
        &mut blockers,
    );
    if !blockers.is_empty() {
        return Err(blockers);
    }

    let source = FrozenMappingSource {
        version: FROZEN_MAPPING_SOURCE_VERSION,
        formula_source_uuid: source_ref.uuid(),
        formula,
        formula_instance,
        authored_regions,
        commands,
    };
    source
        .compile_runtime()
        .map_err(|detail| vec![FrozenMappingBlocker::new(
            "compile_failed",
            snapshot.node(processor).map(|node| node.uuid),
            detail,
        )])?;
    Ok(source)
}

fn prepare_frozen_command(
    snapshot: &ProcessTreeSnapshot,
    command: NodeId,
) -> Result<FrozenCommandDefinition, FrozenMappingBlocker> {
    let node = snapshot.node(command).ok_or_else(|| {
        FrozenMappingBlocker::new("missing_command", None, "Output command disappeared during preparation")
    })?;
    if !is_output_node(snapshot, command) {
        return Err(FrozenMappingBlocker::new(
            "unsupported_output",
            Some(node.uuid),
            format!("'{}' is not a directly executable command", node.label),
        ));
    }

    let operation = match node.node_type.as_str() {
        GENERIC_SET_PARAMETER_COMMAND_NODE_TYPE => FrozenGenericCommandOperation::SetParameter {
            target_parameter: command_parameter_uuid(snapshot, command, "target")?,
            value_parameter: command_parameter_uuid(snapshot, command, "value")?,
        },
        GENERIC_TRIGGER_PARAMETER_COMMAND_NODE_TYPE => {
            FrozenGenericCommandOperation::TriggerParameter {
                target_parameter: command_parameter_uuid(snapshot, command, "target")?,
            }
        }
        _ => {
            return Err(FrozenMappingBlocker::new(
                "unsupported_command",
                Some(node.uuid),
                format!(
                    "command type '{}' requires a live authored node and cannot be compressed",
                    node.node_type
                ),
            ));
        }
    };

    let mut parameters = Vec::new();
    collect_command_parameters(snapshot, command, &mut parameters)?;
    let bindings = mapping_output_binding_config(snapshot, command).map_err(|detail| {
        FrozenMappingBlocker::new("invalid_command_binding", Some(node.uuid), detail)
    })?;
    Ok(FrozenCommandDefinition {
        uuid: node.uuid,
        label: node.label.clone(),
        enabled: node.enabled,
        bindings,
        parameters,
        operation,
    })
}

fn command_parameter_uuid(
    snapshot: &ProcessTreeSnapshot,
    command: NodeId,
    decl_id: &str,
) -> Result<NodeUuid, FrozenMappingBlocker> {
    let command_uuid = snapshot.node(command).map(|node| node.uuid);
    module_command::resolve_module_command_child(snapshot, command, decl_id)
        .and_then(|parameter| snapshot.node(parameter))
        .filter(|parameter| parameter.param_value.is_some())
        .map(|parameter| parameter.uuid)
        .ok_or_else(|| {
            FrozenMappingBlocker::new(
                "missing_command_parameter",
                command_uuid,
                format!("command parameter '{decl_id}' is missing"),
            )
        })
}

fn collect_command_parameters(
    snapshot: &ProcessTreeSnapshot,
    node: NodeId,
    parameters: &mut Vec<FrozenCommandParameter>,
) -> Result<(), FrozenMappingBlocker> {
    let Some(current) = snapshot.node(node) else {
        return Ok(());
    };
    if let Some(value) = current.param_value.as_ref() {
        if current
            .param_control
            .as_ref()
            .is_some_and(|control| control.mode != ParameterControlMode::Manual)
        {
            return Err(FrozenMappingBlocker::new(
                "live_parameter_control",
                Some(current.uuid),
                format!(
                    "parameter '{}' uses a live control mode that requires an authored node",
                    current.label
                ),
            ));
        }
        let mut value = value.clone();
        if let ParamValue::Reference(reference) = &mut value {
            reference.clear_cached_id();
        }
        parameters.push(FrozenCommandParameter {
            uuid: current.uuid,
            decl_id: current.decl_id.clone(),
            value,
        });
    }
    for child in snapshot.child_ids(node) {
        collect_command_parameters(snapshot, child, parameters)?;
    }
    Ok(())
}

fn collect_subtree(
    snapshot: &ProcessTreeSnapshot,
    root: NodeId,
    nodes: &mut HashSet<NodeId>,
    uuids: &mut HashSet<NodeUuid>,
) {
    let Some(node) = snapshot.node(root) else {
        return;
    };
    nodes.insert(root);
    uuids.insert(node.uuid);
    for child in snapshot.child_ids(root) {
        collect_subtree(snapshot, child, nodes, uuids);
    }
}

fn collect_inbound_reference_blockers(
    engine: &AppEngine,
    workflow_nodes: &HashSet<NodeId>,
    workflow_uuids: &HashSet<NodeUuid>,
    blockers: &mut Vec<FrozenMappingBlocker>,
) {
    for (node_id, node) in engine.nodes.iter() {
        if workflow_nodes.contains(&node_id) {
            continue;
        }
        node.engine_visit_references(&mut |reference| {
            if workflow_uuids.contains(&reference.uuid()) {
                blockers.push(FrozenMappingBlocker::new(
                    "inbound_reference",
                    Some(node.node_data().meta.uuid),
                    format!(
                        "'{}' references archived workflow node {}; expand or remove the reference before compression",
                        node.node_data().meta.label,
                        reference.uuid().0
                    ),
                ));
            }
        });
    }
}

fn persistence_blocker(
    snapshot: &ProcessTreeSnapshot,
    region: NodeId,
    error: ProjectPersistenceError,
) -> FrozenMappingBlocker {
    FrozenMappingBlocker::new(
        "archive_failed",
        snapshot.node(region).map(|node| node.uuid),
        error.to_string(),
    )
}

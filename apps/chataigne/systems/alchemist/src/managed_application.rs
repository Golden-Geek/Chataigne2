//! Instance-aware managed filter contracts resolved from the same ANode declaration as graph use.

use crate::{
    ANodeDeclaration, ANodeInstance, ANodeSignature, ANodeTypeId, AutoWirePolicy, ChannelGroups, ChannelLayout,
    ChannelLayoutError, ChannelSelection, ChannelSelectionResolution, ManagedSettingClass, PipelineCardinality,
    RuntimeValue, SignatureCtx, SocketId, SurfaceItemKind, TypeConstraint, ValueLaneKey, ValueTypeId,
};

pub const MANAGED_SELECTION_FIELD: &str = "managed_selection";
/// Internal stage lowering marks a synthesized default connection so HoldLast can still
/// distinguish it from an explicit authored default edge in a Formula graph.
pub const MANAGED_IMPLICIT_GATE_DEFAULT_FIELD: &str = "_managed_implicit_gate_default";
pub const MANAGED_GROUPS_FIELD: &str = "managed_groups";
/// App-authored managed filters set this transient flag when their optional
/// input-count control is disabled. The runtime specializes that declaration to
/// the current ordered tuple without rewriting the authored control.
pub const MANAGED_AUTO_INPUT_COUNT_FIELD: &str = "_managed_auto_input_count";

/// Resolve the same configured variant for palette validation and backend creation.
/// A sized variant is valid only for a declared aggregate with a `num_inputs` setting.
pub fn configured_managed_variant(
    declaration: &dyn ANodeDeclaration,
    index: usize,
    input_count: Option<usize>,
) -> Option<ANodeInstance> {
    let mut instance = declaration.managed_application_variants().into_iter().nth(index)?;
    if let Some(input_count) = input_count {
        if !(2..=64).contains(&input_count)
            || !declaration.role_capabilities_for(&instance).iter().any(|capability| {
                capability.role == SurfaceItemKind::Filter && capability.cardinality == PipelineCardinality::Aggregate
            })
            || !declaration
                .config_fields_for(&instance)
                .iter()
                .any(|field| field.id.as_str() == "num_inputs")
        {
            return None;
        }
        instance.config.set("num_inputs", RuntimeValue::Int(input_count as i64));
    }
    Some(instance)
}

#[derive(Clone, Copy, Debug)]
pub enum ManagedSettingPath<'a> {
    Input(&'a SocketId),
    Config(&'a str),
    Presentation,
}

pub fn classify_managed_setting(
    declaration: &dyn ANodeDeclaration,
    instance: &ANodeInstance,
    ctx: &SignatureCtx<'_>,
    path: ManagedSettingPath<'_>,
) -> Result<ManagedSettingClass, ManagedApplicationError> {
    match path {
        ManagedSettingPath::Presentation => Ok(ManagedSettingClass::Presentation),
        ManagedSettingPath::Config(field) => declaration
            .config_fields_for(instance)
            .into_iter()
            .find(|candidate| candidate.id.as_str() == field)
            .map(|field| field.update_class)
            .ok_or_else(|| ManagedApplicationError::UnknownSetting(field.to_owned())),
        ManagedSettingPath::Input(socket) => declaration
            .signature(ctx, instance, &instance.type_bindings)
            .inputs
            .iter()
            .any(|input| input.id == *socket)
            .then_some(ManagedSettingClass::RuntimeValue)
            .ok_or_else(|| ManagedApplicationError::UnknownSetting(socket.to_string())),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ManagedStateScope {
    PerChannel,
    PerGroup,
    WholeStream,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ManagedApplication {
    pub cardinality: PipelineCardinality,
    pub primary_inputs: Vec<SocketId>,
    pub auxiliary_inputs: Vec<SocketId>,
    pub outputs: Vec<SocketId>,
    pub selection: ChannelSelectionResolution,
    /// Groups and members follow authored order; a default reduction uses selected order.
    pub groups: Vec<Vec<usize>>,
    pub state_scope: ManagedStateScope,
}

pub fn resolve_managed_application(
    declaration: &dyn ANodeDeclaration,
    instance: &ANodeInstance,
    layout: &ChannelLayout,
    ctx: &SignatureCtx<'_>,
) -> Result<ManagedApplication, ManagedApplicationError> {
    let capability = declaration
        .role_capabilities_for(instance)
        .into_iter()
        .find(|capability| capability.role == SurfaceItemKind::Filter)
        .ok_or(ManagedApplicationError::NotFilterCapable)?;
    let signature = declaration.signature(ctx, instance, &instance.type_bindings);
    let primary_inputs = if let Some(input) = capability.primary_input {
        if !signature.inputs.iter().any(|candidate| candidate.id == input) {
            return Err(ManagedApplicationError::UnknownPrimaryInput(input));
        }
        vec![input]
    } else {
        signature.inputs.iter().map(|input| input.id.clone()).collect()
    };
    if primary_inputs.is_empty() {
        return Err(ManagedApplicationError::NoPrimaryInputs);
    }
    let auxiliary_inputs: Vec<_> = signature
        .inputs
        .iter()
        .filter(|input| !primary_inputs.contains(&input.id))
        .map(|input| input.id.clone())
        .collect();
    for input in &signature.inputs {
        if !auxiliary_inputs.contains(&input.id) {
            continue;
        }
        if let Some(value) = instance.input_defaults.get(&input.id) {
            let actual = match value {
                RuntimeValue::Ref(reference) => reference.value_type.clone(),
                _ => value.value_type(),
            };
            if !accepts_constraint(&input.constraint, &signature, &actual, ctx) {
                return Err(ManagedApplicationError::IncompatibleAuxiliaryInput {
                    socket: input.id.clone(),
                    actual,
                });
            }
        }
    }
    let mut outputs: Vec<_> = signature.outputs.iter().map(|output| output.id.clone()).collect();
    if outputs.is_empty() {
        return Err(ManagedApplicationError::NoOutputs);
    }
    if let Some(primary) = capability.primary_output
        && !outputs.contains(&primary)
    {
        return Err(ManagedApplicationError::UnknownPrimaryOutput(primary));
    }
    if let AutoWirePolicy::Gate { output, .. } = &capability.autowire {
        outputs = vec![output.clone()];
    }

    let selection = parse_selection(instance)?;
    let first_constraint = signature
        .inputs
        .iter()
        .find(|input| input.id == primary_inputs[0])
        .expect("primary input was checked against the signature");
    let accepts =
        |value_type: &ValueTypeId| accepts_constraint(&first_constraint.constraint, &signature, value_type, ctx);
    let selection = selection.resolve(layout, accepts)?;
    let per_channel = capability.cardinality == PipelineCardinality::Elementwise
        || matches!(&capability.autowire, AutoWirePolicy::Gate { .. })
        || (capability.cardinality == PipelineCardinality::Reshape && primary_inputs.len() == 1 && outputs.len() > 1);
    let groups = match parse_groups(instance)? {
        Some(groups) => {
            let resolved = groups.resolve(layout, accepts)?;
            if resolved
                .iter()
                .flatten()
                .any(|index| !selection.indices.contains(index))
            {
                return Err(ManagedApplicationError::GroupOutsideSelection);
            }
            resolved
        }
        None if selection.indices.is_empty() => Vec::new(),
        None if per_channel => selection.indices.iter().map(|index| vec![*index]).collect(),
        None => vec![selection.indices.clone()],
    };
    if !selection.indices.is_empty()
        && matches!(
            capability.cardinality,
            PipelineCardinality::Aggregate | PipelineCardinality::Reshape
        )
    {
        for group in &groups {
            if group.len() != primary_inputs.len() {
                return Err(ManagedApplicationError::GroupArityMismatch {
                    expected: primary_inputs.len(),
                    actual: group.len(),
                });
            }
        }
    }
    let state_scope = match capability.cardinality {
        PipelineCardinality::Elementwise => ManagedStateScope::PerChannel,
        PipelineCardinality::Reshape if per_channel => ManagedStateScope::PerChannel,
        PipelineCardinality::Aggregate | PipelineCardinality::Reshape | PipelineCardinality::Expand => {
            ManagedStateScope::PerGroup
        }
        PipelineCardinality::WholeSet if matches!(&capability.autowire, AutoWirePolicy::Gate { .. }) => {
            ManagedStateScope::PerChannel
        }
        PipelineCardinality::WholeSet => ManagedStateScope::WholeStream,
    };
    Ok(ManagedApplication {
        cardinality: capability.cardinality,
        primary_inputs,
        auxiliary_inputs,
        outputs,
        selection,
        groups,
        state_scope,
    })
}

pub fn resolve_mapping_application(
    declaration: &dyn ANodeDeclaration,
    instance: &ANodeInstance,
    layout: &ChannelLayout,
    ctx: &SignatureCtx<'_>,
) -> Result<ManagedApplication, ManagedApplicationError> {
    if instance.config.get(MANAGED_SELECTION_FIELD).is_some() || instance.config.get(MANAGED_GROUPS_FIELD).is_some() {
        return Err(ManagedApplicationError::ExplicitChannelRouting);
    }
    let application = resolve_managed_application(declaration, instance, layout, ctx)?;
    if application.selection.indices.len() != layout.channels().len() {
        return Err(ManagedApplicationError::IncompatibleTuple);
    }
    Ok(application)
}

fn accepts_constraint(
    constraint: &TypeConstraint,
    signature: &ANodeSignature,
    value_type: &ValueTypeId,
    ctx: &SignatureCtx<'_>,
) -> bool {
    match constraint {
        TypeConstraint::Exact(expected) => expected == value_type,
        TypeConstraint::Generic(variable) => signature
            .generic_constraints
            .get(variable)
            .is_some_and(|constraint| accepts_constraint(constraint, signature, value_type, ctx)),
        TypeConstraint::OneOf(options) => options
            .iter()
            .any(|constraint| accepts_constraint(constraint, signature, value_type, ctx)),
        _ => constraint.accepts_value_type(value_type, ctx.value_types),
    }
}

fn parse_selection(instance: &ANodeInstance) -> Result<ChannelSelection, ManagedApplicationError> {
    match instance.config.get(MANAGED_SELECTION_FIELD) {
        None => Ok(ChannelSelection::AllCompatible),
        Some(RuntimeValue::String(value)) if value.as_ref() == "all" => Ok(ChannelSelection::AllCompatible),
        Some(RuntimeValue::Array(values)) if !values.is_empty() => Ok(ChannelSelection::Explicit(parse_ids(values)?)),
        _ => Err(ManagedApplicationError::InvalidSelectionConfig),
    }
}

fn parse_groups(instance: &ANodeInstance) -> Result<Option<ChannelGroups>, ManagedApplicationError> {
    let Some(value) = instance.config.get(MANAGED_GROUPS_FIELD) else {
        return Ok(None);
    };
    let RuntimeValue::Array(groups) = value else {
        return Err(ManagedApplicationError::InvalidGroupsConfig);
    };
    if groups.is_empty() {
        return Err(ManagedApplicationError::InvalidGroupsConfig);
    }
    let groups = groups
        .iter()
        .map(|group| match group {
            RuntimeValue::Array(values) => parse_ids(values),
            _ => Err(ManagedApplicationError::InvalidGroupsConfig),
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Some(ChannelGroups(groups)))
}

fn parse_ids(values: &[RuntimeValue]) -> Result<Vec<ValueLaneKey>, ManagedApplicationError> {
    values
        .iter()
        .map(|value| match value {
            RuntimeValue::String(id) => ValueLaneKey::new(id.as_ref()).map_err(ManagedApplicationError::from),
            _ => Err(ManagedApplicationError::InvalidSelectionConfig),
        })
        .collect()
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ManagedApplicationError {
    #[error("managed setting `{0}` is absent from the configured ANode declaration")]
    UnknownSetting(String),
    #[error("ANode declaration `{0}` is not registered")]
    MissingDeclaration(ANodeTypeId),
    #[error("the configured ANode is not filter-capable")]
    NotFilterCapable,
    #[error("the managed application has no primary inputs")]
    NoPrimaryInputs,
    #[error("the managed application has no output sockets")]
    NoOutputs,
    #[error("the configured primary input `{0}` is absent from the ANode signature")]
    UnknownPrimaryInput(SocketId),
    #[error("the configured primary output `{0}` is absent from the ANode signature")]
    UnknownPrimaryOutput(SocketId),
    #[error("managed selection must be `all` or an array of stable channel IDs")]
    InvalidSelectionConfig,
    #[error("managed groups must be arrays of stable channel IDs")]
    InvalidGroupsConfig,
    #[error("auxiliary input `{socket}` cannot accept `{actual}`")]
    IncompatibleAuxiliaryInput { socket: SocketId, actual: ValueTypeId },
    #[error("a managed group contains a channel outside the application selection")]
    GroupOutsideSelection,
    #[error("managed group has {actual} channels but the ANode signature requires {expected}")]
    GroupArityMismatch { expected: usize, actual: usize },
    #[error("standard Mapping filters consume the whole value; use a custom Formula for channel selections or groups")]
    ExplicitChannelRouting,
    #[error("the filter cannot consume every element of the ordered input tuple")]
    IncompatibleTuple,
    #[error("{0}")]
    Layout(#[from] ChannelLayoutError),
}

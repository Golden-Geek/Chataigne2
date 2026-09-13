//! Palette availability is checked against the same managed compiler used at runtime.

use chataigne_alchemist::{
    ANodeInstance, ChannelLayout, CompileCtx, MANAGED_GROUPS_FIELD, ManagedApplication, ManagedApplicationError,
    ManagedItemId, ManagedItemInstance, ManagedItemUiState, SignatureCtx,
};

use super::{ManagedFilterCompileKey, ManagedFilterCompiledRuntime, ManagedFilterPipelineRuntime, ManagedFormulaError};

pub struct ExecutableFilterApplication {
    pub instance: ANodeInstance,
    pub application: ManagedApplication,
}

#[derive(Debug, thiserror::Error)]
pub enum ManagedFilterAvailabilityError {
    #[error("{0}")]
    Application(#[from] ManagedApplicationError),
    #[error("managed filter cannot execute until every input channel has a declared type")]
    UnresolvedInputType,
    #[error("the current managed runner requires one input type across the layout")]
    MixedInputTypes,
    #[error("the current managed runner cannot execute an authored channel subset or group")]
    SelectionOrGroup,
    #[error("{0}")]
    Compile(#[from] ManagedFormulaError),
}

pub fn validate_executable_filter_application(
    instance: &ANodeInstance,
    layout: &ChannelLayout,
    ctx: &CompileCtx<'_>,
) -> Result<ManagedApplication, ManagedFilterAvailabilityError> {
    let signature_ctx = SignatureCtx {
        value_types: ctx.value_types,
        properties: ctx.properties,
    };
    let application = ctx
        .nodes
        .resolve_managed_application(instance, layout, &signature_ctx)?;
    let channels = layout.channels();
    let first_type = channels
        .first()
        .and_then(|channel| channel.value_type.clone())
        .ok_or(ManagedFilterAvailabilityError::UnresolvedInputType)?;
    if channels.iter().any(|channel| channel.value_type.is_none()) {
        return Err(ManagedFilterAvailabilityError::UnresolvedInputType);
    }
    if channels
        .iter()
        .any(|channel| channel.value_type.as_ref() != Some(&first_type))
    {
        return Err(ManagedFilterAvailabilityError::MixedInputTypes);
    }
    if application.selection.indices.iter().copied().ne(0..channels.len())
        || instance.config.get(MANAGED_GROUPS_FIELD).is_some()
    {
        return Err(ManagedFilterAvailabilityError::SelectionOrGroup);
    }
    let item = ManagedItemInstance {
        id: ManagedItemId::new(),
        anode: instance.clone(),
        enabled: true,
        ui_state: ManagedItemUiState::default(),
    };
    let runtime = ManagedFilterPipelineRuntime {
        definition: None,
        instance: None,
        value_types: ctx.value_types.clone(),
        nodes: ctx.nodes.clone(),
        compiled_key: None,
        compiled: ManagedFilterCompiledRuntime::PassThrough,
    };
    runtime.compile_for_key(
        &[item],
        &ManagedFilterCompileKey {
            item_type: first_type,
            lane_count: channels.len(),
        },
    )?;
    Ok(application)
}

pub fn executable_filter_applications(
    layout: &ChannelLayout,
    ctx: &CompileCtx<'_>,
) -> Vec<ExecutableFilterApplication> {
    ctx.nodes
        .iter()
        .flat_map(|declaration| declaration.managed_application_variants())
        .filter_map(|instance| {
            validate_executable_filter_application(&instance, layout, ctx)
                .ok()
                .map(|application| ExecutableFilterApplication { instance, application })
        })
        .collect()
}

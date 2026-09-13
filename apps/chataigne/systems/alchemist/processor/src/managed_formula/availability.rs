//! Palette availability is checked against the same typed stage compiler used at runtime.

use chataigne_alchemist::{
    ANodeInstance, ChannelLayout, CompileCtx, ManagedApplication, ManagedApplicationError, ManagedFilterValueMode,
    ManagedItemId, ManagedItemInstance, ManagedItemUiState, SignatureCtx,
};

use crate::{ManagedStageError, ManagedStageRuntime};

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
    #[error("the filter selects no compatible channels")]
    NoCompatibleChannels,
    #[error("{0}")]
    Compile(#[from] ManagedStageError),
}

pub fn validate_executable_filter_application(
    instance: &ANodeInstance,
    layout: &ChannelLayout,
    ctx: &CompileCtx<'_>,
) -> Result<ManagedApplication, ManagedFilterAvailabilityError> {
    validate_filter_application(instance, layout, ctx, ManagedFilterValueMode::Routed)
}

pub fn validate_mapping_filter_application(
    instance: &ANodeInstance,
    layout: &ChannelLayout,
    ctx: &CompileCtx<'_>,
) -> Result<ManagedApplication, ManagedFilterAvailabilityError> {
    validate_filter_application(instance, layout, ctx, ManagedFilterValueMode::Tuple)
}

fn validate_filter_application(
    instance: &ANodeInstance,
    layout: &ChannelLayout,
    ctx: &CompileCtx<'_>,
    mode: ManagedFilterValueMode,
) -> Result<ManagedApplication, ManagedFilterAvailabilityError> {
    if layout.channels().iter().any(|channel| channel.value_type.is_none()) {
        return Err(ManagedFilterAvailabilityError::UnresolvedInputType);
    }
    let signature_ctx = SignatureCtx {
        value_types: ctx.value_types,
        properties: ctx.properties,
    };
    let application = match mode {
        ManagedFilterValueMode::Tuple => ctx
            .nodes
            .resolve_mapping_application(instance, layout, &signature_ctx)?,
        ManagedFilterValueMode::Routed => ctx
            .nodes
            .resolve_managed_application(instance, layout, &signature_ctx)?,
    };
    let item = ManagedItemInstance {
        id: ManagedItemId::new(),
        anode: instance.clone(),
        enabled: true,
        ui_state: ManagedItemUiState::default(),
    };
    ManagedStageRuntime::compile(item, layout, ctx, mode)?
        .ok_or(ManagedFilterAvailabilityError::NoCompatibleChannels)?;
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

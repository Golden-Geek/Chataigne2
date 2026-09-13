use chataigne_alchemist::{
    ANodeTypeId, Diagnostic, DiagnosticOrigin, FormulaMaterializationError, ManagedItemId, ManagedRegionId,
    ManagedRegionKind, ManagedRegionValidationError, ValueTypeId,
};

use crate::{
    INPUT_SOURCE_FIELD, InputSetError, ManagedStageError, OUTPUT_TARGET_FIELD, OutputSetError, ValueSetError,
    ValueSetPipelineError,
};

#[derive(Debug, thiserror::Error)]
pub enum ManagedFormulaError {
    #[error("managed filter input types are unresolved; bind a source schema before evaluation")]
    UnresolvedManagedInputSchema,
    #[error("managed stage channel `{0:?}` is not valid for positional output dispatch")]
    InvalidStageChannel(chataigne_alchemist::ValueLaneKey),
    #[error("{0}")]
    ManagedStage(#[from] ManagedStageError),
    #[error("managed Formula graph cannot compile: {0:?}")]
    GraphCompile(Vec<Diagnostic>),
    #[error("managed Formula graph boundary is invalid: {0}")]
    GraphBoundary(String),
    #[error("managed filter item `{0}` does not exist")]
    MissingFilterItem(ManagedItemId),
    #[error("{0}")]
    Formula(#[from] FormulaMaterializationError),
    #[error("{0}")]
    ManagedRegionValidation(#[from] ManagedRegionValidationError),
    #[error("managed formula declares both value and trigger pipeline region families")]
    MixedManagedFormulaPipelines,
    #[error("managed formula is missing a `{kind:?}` region")]
    MissingRegion { kind: ManagedRegionKind },
    #[error("managed formula declares more than one `{kind:?}` region")]
    DuplicateRegion { kind: ManagedRegionKind },
    #[error("managed region `{region_id}` has no instance")]
    MissingRegionInstance { region_id: ManagedRegionId },
    #[error("{0}")]
    InputSet(#[from] InputSetError),
    #[error("{0}")]
    OutputSet(#[from] OutputSetError),
    #[error("managed trigger input region `{region_id}` is `{actual:?}`, expected TriggerInput")]
    WrongTriggerInputRegionKind {
        region_id: ManagedRegionId,
        actual: ManagedRegionKind,
    },
    #[error("TriggerInput region `{region_id}` must accept input items")]
    DoesNotAcceptTriggerInputs { region_id: ManagedRegionId },
    #[error("TriggerInput item `{label}` is missing a `{INPUT_SOURCE_FIELD}` StableRef config field")]
    MissingTriggerInputSourceConfig { label: String },
    #[error("TriggerInput item `{label}` has non-reference `{INPUT_SOURCE_FIELD}` config value `{actual}`")]
    InvalidTriggerInputSourceConfig { label: String, actual: String },
    #[error("managed command set region `{region_id}` is `{actual:?}`, expected CommandSet")]
    WrongCommandSetRegionKind {
        region_id: ManagedRegionId,
        actual: ManagedRegionKind,
    },
    #[error("CommandSet region `{region_id}` must accept command items")]
    DoesNotAcceptCommands { region_id: ManagedRegionId },
    #[error("CommandSet item `{label}` is missing a `{OUTPUT_TARGET_FIELD}` StableRef config field")]
    MissingCommandTargetConfig { label: String },
    #[error("CommandSet item `{label}` has non-reference `{OUTPUT_TARGET_FIELD}` config value `{actual}`")]
    InvalidCommandTargetConfig { label: String, actual: String },
    #[error("managed filter region `{region_id}` is `{actual:?}`, expected FilterPipeline")]
    WrongFilterRegionKind {
        region_id: ManagedRegionId,
        actual: ManagedRegionKind,
    },
    #[error("managed region instance `{instance_id}` does not match definition `{definition_id}`")]
    RegionMismatch {
        definition_id: ManagedRegionId,
        instance_id: ManagedRegionId,
    },
    #[error("FilterPipeline region `{region_id}` must accept filter items")]
    DoesNotAcceptFilters { region_id: ManagedRegionId },
    #[error("managed filter item declaration `{node_type}` is not registered")]
    MissingFilterDeclaration { node_type: ANodeTypeId },
    #[error("managed filter shape is invalid: {}", messages.join("; "))]
    InvalidFilterShape { messages: Vec<String> },
    #[error("unsupported managed filter pipeline: {0}")]
    UnsupportedFilterPipeline(String),
    #[error("managed filter pipeline requires at least one input value")]
    EmptyFilteredValueSet,
    #[error("managed ValueSet contains mixed value types `{expected}` and `{actual}`")]
    MixedValueSetTypes { expected: ValueTypeId, actual: ValueTypeId },
    #[error("Trigger filter expected one trigger value, got {actual}")]
    TriggerFilterExpectedSingleValue { actual: usize },
    #[error("managed filter produced diagnostics: {}", messages.join("; "))]
    FilterDiagnostics { messages: Vec<String> },
    #[error("{0}")]
    ValueSet(#[from] ValueSetError),
    #[error("{0}")]
    ValueSetPipeline(#[from] ValueSetPipelineError),
}

impl ManagedFormulaError {
    pub fn into_diagnostic(self) -> Diagnostic {
        Diagnostic::error(self.diagnostic_code(), self.to_string(), DiagnosticOrigin::Graph)
    }

    pub(super) fn diagnostic_code(&self) -> &'static str {
        match self {
            Self::UnresolvedManagedInputSchema => "managed_formula_unresolved_input_schema",
            Self::InvalidStageChannel(_) => "managed_formula_invalid_stage_channel",
            Self::ManagedStage(_) => "managed_formula_stage_error",
            Self::GraphCompile(_) => "managed_formula_graph_compile",
            Self::GraphBoundary(_) => "managed_formula_graph_boundary",
            Self::Formula(_) => "managed_formula_materialization_error",
            Self::ManagedRegionValidation(_) => "managed_formula_region_validation_error",
            Self::MixedManagedFormulaPipelines => "managed_formula_mixed_region_kinds",
            Self::MissingRegion { .. } => "managed_formula_missing_region",
            Self::DuplicateRegion { .. } => "managed_formula_duplicate_region",
            Self::MissingRegionInstance { .. } => "managed_formula_missing_region_instance",
            Self::InputSet(_) => "managed_formula_input_set_error",
            Self::OutputSet(_) => "managed_formula_output_set_error",
            Self::WrongTriggerInputRegionKind { .. } => "managed_formula_wrong_trigger_input_region_kind",
            Self::DoesNotAcceptTriggerInputs { .. } => "managed_formula_trigger_input_role_rejected",
            Self::MissingTriggerInputSourceConfig { .. } => "managed_formula_missing_trigger_input_source",
            Self::InvalidTriggerInputSourceConfig { .. } => "managed_formula_invalid_trigger_input_source",
            Self::WrongCommandSetRegionKind { .. } => "managed_formula_wrong_command_set_region_kind",
            Self::DoesNotAcceptCommands { .. } => "managed_formula_command_set_role_rejected",
            Self::MissingCommandTargetConfig { .. } => "managed_formula_missing_command_target",
            Self::InvalidCommandTargetConfig { .. } => "managed_formula_invalid_command_target",
            Self::WrongFilterRegionKind { .. } => "managed_formula_wrong_filter_region_kind",
            Self::RegionMismatch { .. } => "managed_formula_region_mismatch",
            Self::DoesNotAcceptFilters { .. } => "managed_formula_filter_role_rejected",
            Self::MissingFilterDeclaration { .. } => "managed_formula_missing_filter_declaration",
            Self::MissingFilterItem(_) => "managed_formula_missing_filter_item",
            Self::InvalidFilterShape { .. } => "managed_formula_invalid_filter_shape",
            Self::UnsupportedFilterPipeline(_) => "managed_formula_unsupported_filter_pipeline",
            Self::EmptyFilteredValueSet => "managed_formula_empty_filtered_valueset",
            Self::MixedValueSetTypes { .. } => "managed_formula_mixed_valueset_types",
            Self::TriggerFilterExpectedSingleValue { .. } => "managed_formula_trigger_filter_expected_single_value",
            Self::FilterDiagnostics { .. } => "managed_formula_filter_diagnostics",
            Self::ValueSet(_) => "managed_formula_valueset_error",
            Self::ValueSetPipeline(_) => "managed_formula_valueset_pipeline_error",
        }
    }
}

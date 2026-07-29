//! Chataigne-owned Alchemist formula authoring, compilation, and runtime.

pub mod compile;
pub mod diagnostics;
pub mod domain;
pub mod expose;
pub mod formula;
pub mod graph;
pub mod ids;
pub mod library;
pub mod node;
pub mod pipeline;
pub mod registry;
pub mod runtime;
pub mod typing;
pub mod value;

pub use diagnostics::{Diagnostic, DiagnosticOrigin, DiagnosticSeverity};
pub use domain::{
    AlchemistEdgeData, AlchemistGraphData, AlchemistGraphDocument, AlchemistGraphDomain, AlchemistGraphEnvelope,
    AlchemistGraphSemanticError, AlchemistGraphTransaction, AlchemistGraphTransactionError, AlchemistNodeData,
    AlchemistPortData,
};
pub use expose::{
    ANodeFieldPath, ExposedAction, ExposedInput, ExposedOutput, ExposedParam, ExposedSurface, ParamUiHints,
    ValueTypeSpec,
};
pub use formula::{
    AlchemistFormula, AlchemistFormulaInstance, FormulaContextContract, FormulaMaterializationError, FormulaMigration,
    FormulaOverrides, FormulaPropertyDecl, FormulaPropertySchema, FormulaRef, FormulaSurface, FormulaSurfaceBindings,
    ManagedANodeBindings, ManagedItemInstance, ManagedItemUiState, ManagedRegionDefinition, ManagedRegionInstance,
    ManagedRegionInstances, ManagedRegionKind, ManagedRegionValidationError, ManagedSocketRef, PropertyUiHints,
    SurfaceItem, SurfaceItemKind, SurfaceSection, SurfaceSource,
};
pub use graph::{AEdge, ANodeConfig, ANodeInstance, ANodeUiState, GraphMetadata, InputSocketRef, OutputSocketRef};
pub use ids::{
    ANodeId, ANodeTypeId, AlchemistGraphId, ContextDimensionId, ExecNodeId, ExposedDeclId, FacetId, FormulaId,
    FormulaPropertyId, FormulaPropertySlotId, ManagedItemId, ManagedRegionId, SocketId, SurfaceContributionId,
    SurfaceItemId, SurfaceSectionId, ValueSlotId, ValueTypeId,
};
pub use library::{PrimitiveNodeDeclaration, PrimitiveNodeKind, primitive_node_registry, register_primitive_nodes};
pub use node::{
    ANodeConfigFieldDecl, ANodeDeclaration, ANodeRoleCapability, ANodeSignature, AutoWirePolicy, ExecutionKind,
    InputSocketDecl, ManagedUiMode, NodeStateLayout, OutputSocketDecl, PROCESS_ON_INPUT_CHANGE_ONLY_CONFIG,
    PipelineCardinality, SEND_ON_OUTPUT_CHANGE_ONLY_CONFIG, SignatureCtx,
};
pub use pipeline::{
    FilterPipelineLoweringResult, PipelineLoweringCtx, PipelineLoweringDiagnostic, PipelineLoweringDiagnosticKind,
    PipelineShape, PipelineShapeCheckItem, PipelineShapeDiagnostic, PipelineShapeResult, PipelineShapeStep,
    check_filter_pipeline_shapes, lower_filter_pipeline_region, single_shape, value_set_shape,
};
pub use registry::{
    ANodeRegistry, ConversionKind, ConversionRule, FacetDescriptor, FacetRegistry, RegistryError, ValueTypeDescriptor,
    ValueTypeRegistry, ValueTypeUiDescriptor,
};
pub use runtime::{
    AlchemistMemory, AlchemistRuntime, AxisSet, CompiledNodeEvaluator, ContextAxisId, ContextItemId, ContextKey,
    ContextKeyPart, ContextValuePath, DebugCaptureMode, DebugCaptureSink, DebugValueSample, EvaluationCtx,
    EvaluationFrame, LaneRuntimePool, NodeEvaluation, OutputPreviewHistory, OutputPreviewStatus, RuntimeContextFrame,
    RuntimeDiagnostic, RuntimeEvent, RuntimeInputSnapshot, RuntimeIntent, RuntimeOutput, RuntimePropertyFrame,
    RuntimePropertyFrameError, RuntimeRegistries, evaluate_compiled_graph, evaluate_compiled_graph_fresh_reusing,
    evaluate_compiled_graph_stateless,
};
pub use typing::{
    ResolvedANode, ResolvedANodeSignature, ResolvedGraph, ResolvedSocket, TypeBinding, TypeBindingConflict,
    TypeBindingSource, TypeBindings, TypeConstraint, TypeSolveCtx, TypeSolveResult, TypeVar, solve_document_types,
};
pub use value::{
    ColorValue, ExtensionValue, StableRef, TriggerValue, ValueComponent, ValueStorageKind, component_value_type,
};

pub(crate) use golden_values::Value as RuntimeValue;

/// Current authored graph schema version.
pub const ALCHEMIST_SCHEMA_VERSION: u32 = 1;

#[cfg(test)]
mod tests;
pub use compile::{
    CompileCtx, CompileResult, CompiledAlchemistFormula, CompiledAlchemistGraph, CompiledExecNode,
    CompiledFormulaProperty, CompiledFormulaPropertySchema, CompiledNodeOperation, DebugSourceMap, DisabledOutput,
    FORMULA_INPUT_VALUE_TYPE, FormulaAnalysis, FormulaCompileKey, InputValueSource, OutputRoute, RuntimeStateLayout,
    RuntimeSubscription, compile_graph, formula_input_value_ref,
};
#[cfg(test)]
pub(crate) use tests::support as test_support;

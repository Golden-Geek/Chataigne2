//! Alchemist formula authoring, compilation, catalogs, and dense evaluation.

mod builder;
mod catalog;
mod codec;
mod compiler;
mod domain;
mod formula;
mod registry;
mod runtime;

pub use builder::{SingleNodeFormulaSpec, SingleNodeInputSpec, SingleNodeOutputSpec, build_single_node_formula};
pub use catalog::{CatalogEntry, FormulaCatalog, FormulaCatalogError};
pub use codec::{
    FORMULA_FILE_VERSION, FormulaCodecError, FormulaEdgeV1, FormulaFileV1, FormulaNodeV1, decode_formula,
    encode_formula,
};
pub use compiler::{
    CompileError, CompileKey, CompiledFormulaKernel, CompiledOp, ExecNodeId, FormulaCompileCache, FormulaCompiler,
    StateLayout, ValueSlot,
};
pub use domain::{
    ANodeTypeId, AlchemistGraphData, AlchemistGraphDomain, AlchemistNode, AlchemistPort, ConversionPolicy,
};
pub use formula::{
    AlchemistFormula, FormulaDefaults, FormulaId, FormulaMetadata, FormulaProperty, FormulaPropertyId, FormulaSchema,
    FormulaSurface, ManagedRegionDefinition, ManagedRegionId, SurfaceInput, SurfaceItemId, SurfaceOutput,
};
pub use registry::{ANodeCapabilities, ANodeDefinition, ANodeRegistry, BuiltinOperation};
pub use runtime::{
    BatchEvaluationReport, EvaluationOptions, EvaluationReport, FormulaInstance, FormulaRuntimeError,
    ObservationSample, evaluate_batch,
};

#[cfg(test)]
mod tests;

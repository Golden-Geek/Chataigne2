//! Reusable typed graph compilation and runtime primitives for Golden applications.

pub mod diagnostics;
pub mod expose;
pub mod graph;
pub mod ids;
pub mod registry;
#[cfg(feature = "serde")]
pub mod serialize;
pub mod value;

pub use diagnostics::{Diagnostic, DiagnosticOrigin, DiagnosticSeverity};
pub use expose::{
    ANodeFieldPath, ExposedAction, ExposedInput, ExposedOutput, ExposedParam, ExposedSurface, ParamUiHints,
    ValueTypeSpec,
};
pub use graph::{
    AEdge, ANodeConfig, ANodeInstance, ANodeUiState, AlchemistGraph, GraphComment, GraphEditError, GraphGroup,
    GraphLayout, GraphMetadata, InputSocketRef, OutputSocketRef,
};
pub use ids::{ANodeId, ANodeTypeId, AlchemistGraphId, ExecNodeId, ExposedDeclId, FacetId, SocketId, ValueTypeId};
pub use registry::{
    ANodeDeclaration, ANodeRegistry, ConversionKind, ConversionRule, FacetDescriptor, FacetRegistry, RegistryError,
    ValueTypeDescriptor, ValueTypeRegistry, ValueTypeUiDescriptor,
};
pub use value::{ColorValue, ExtensionValue, RuntimeValue, StableRef, TriggerValue, ValueStorageKind};

/// Current authored graph schema version.
pub const ALCHEMIST_SCHEMA_VERSION: u32 = 1;

#[cfg(test)]
mod graph_tests;
#[cfg(test)]
mod serialize_tests;
#[cfg(test)]
mod tests;

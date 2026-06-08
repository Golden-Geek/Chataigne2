//! Reusable typed graph compilation and runtime primitives for Golden applications.

pub mod diagnostics;
pub mod ids;
pub mod registry;
pub mod value;

pub use diagnostics::{Diagnostic, DiagnosticOrigin, DiagnosticSeverity};
pub use ids::{ANodeId, ANodeTypeId, AlchemistGraphId, ExecNodeId, ExposedDeclId, FacetId, SocketId, ValueTypeId};
pub use registry::{
    ANodeDeclaration, ANodeRegistry, ConversionKind, ConversionRule, FacetDescriptor, FacetRegistry, RegistryError,
    ValueTypeDescriptor, ValueTypeRegistry, ValueTypeUiDescriptor,
};
pub use value::{ColorValue, ExtensionValue, RuntimeValue, StableRef, TriggerValue, ValueStorageKind};

/// Current authored graph schema version.
pub const ALCHEMIST_SCHEMA_VERSION: u32 = 1;

#[cfg(test)]
mod tests;

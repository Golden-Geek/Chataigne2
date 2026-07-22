use crate::{ANodeInstance, CompiledNodeOperation, Diagnostic, DiagnosticOrigin};

pub(super) fn operation(instance: &ANodeInstance) -> Result<CompiledNodeOperation, Diagnostic> {
    Err(Diagnostic::error(
        "property_requires_compile_context",
        "property nodes compile through compile_graph so they can bind a property slot",
        DiagnosticOrigin::Node(instance.id),
    ))
}

use crate::{ANodeInstance, CompiledNodeOperation, RuntimeValue};

pub(super) fn operation(instance: &ANodeInstance) -> CompiledNodeOperation {
    CompiledNodeOperation::Constant(
        instance
            .config
            .get("value")
            .cloned()
            .unwrap_or(RuntimeValue::Float(0.0)),
    )
}

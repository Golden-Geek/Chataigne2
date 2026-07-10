use std::collections::BTreeMap;

use golden_model::Revision;

use crate::ANodeTypeId;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BuiltinOperation {
    Constant,
    AddFloat,
    MultiplyFloat,
    PassThrough,
    ConditionGate,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ANodeCapabilities {
    pub pure: bool,
    pub state_slots: u16,
    pub time_dependent: bool,
    pub effectful: bool,
    pub deterministic: bool,
    pub thread_safe: bool,
}

impl ANodeCapabilities {
    pub const PURE: Self = Self {
        pure: true,
        state_slots: 0,
        time_dependent: false,
        effectful: false,
        deterministic: true,
        thread_safe: true,
    };
}

#[derive(Clone, Debug)]
pub struct ANodeDefinition {
    pub node_type: ANodeTypeId,
    pub operation: BuiltinOperation,
    pub capabilities: ANodeCapabilities,
}

#[derive(Clone, Debug)]
pub struct ANodeRegistry {
    revision: Revision,
    definitions: BTreeMap<ANodeTypeId, ANodeDefinition>,
}

impl ANodeRegistry {
    pub fn with_builtins() -> Self {
        let definitions = [
            ("constant", BuiltinOperation::Constant),
            ("add_float", BuiltinOperation::AddFloat),
            ("multiply_float", BuiltinOperation::MultiplyFloat),
            ("pass_through", BuiltinOperation::PassThrough),
            ("condition_gate", BuiltinOperation::ConditionGate),
        ]
        .into_iter()
        .map(|(name, operation)| {
            let node_type = ANodeTypeId(name.into());
            (
                node_type.clone(),
                ANodeDefinition {
                    node_type,
                    operation,
                    capabilities: ANodeCapabilities::PURE,
                },
            )
        })
        .collect();
        Self {
            revision: Revision::new(1),
            definitions,
        }
    }

    pub const fn revision(&self) -> Revision {
        self.revision
    }

    pub fn get(&self, node_type: &ANodeTypeId) -> Option<&ANodeDefinition> {
        self.definitions.get(node_type)
    }

    pub fn register(&mut self, definition: ANodeDefinition) -> bool {
        if self.definitions.contains_key(&definition.node_type) {
            return false;
        }
        let Some(revision) = self.revision.next() else {
            return false;
        };
        self.revision = revision;
        self.definitions.insert(definition.node_type.clone(), definition);
        true
    }
}

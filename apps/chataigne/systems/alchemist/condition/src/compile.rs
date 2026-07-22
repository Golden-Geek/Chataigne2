use sha2::{Digest, Sha256};

use crate::{
    ConditionBehavior, ConditionDefinition, ConditionGroupPolicy, ConditionId, ConditionKind, ConditionOperand,
    ConditionProjection, TypedComparator,
};
use golden_values::StableRef;

#[derive(Clone, Debug, PartialEq)]
pub enum CompiledConditionInstruction {
    Constant(bool),
    InputValue {
        input: StableRef,
        projection: ConditionProjection,
        comparator: TypedComparator,
        expected: ConditionOperand,
        expected_max: Option<ConditionOperand>,
        behavior: ConditionBehavior,
        state_slot: usize,
    },
    InputNode {
        provider: String,
        node: StableRef,
        projection: ConditionProjection,
        comparator: TypedComparator,
        expected: ConditionOperand,
        expected_max: Option<ConditionOperand>,
        behavior: ConditionBehavior,
        state_slot: usize,
    },
    Group {
        policy: ConditionGroupPolicy,
        children: Vec<usize>,
    },
    Script {
        script: String,
        behavior: ConditionBehavior,
        state_slot: usize,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ConditionStateKey {
    pub condition: ConditionId,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ConditionStateLayout {
    pub keys: Vec<ConditionStateKey>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConditionObservationDescriptor {
    pub condition: ConditionId,
    pub instruction: usize,
    pub label: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConditionCompileDiagnostic {
    pub condition: ConditionId,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompiledConditionProgram {
    pub instructions: Vec<CompiledConditionInstruction>,
    pub root: usize,
    pub state_layout: ConditionStateLayout,
    pub observation: Vec<ConditionObservationDescriptor>,
    pub kernel_key: [u8; 32],
}

pub fn compile_condition(
    definition: &ConditionDefinition,
) -> Result<CompiledConditionProgram, Vec<ConditionCompileDiagnostic>> {
    let mut compiler = Compiler::default();
    let root = compiler.compile(definition);
    if !compiler.diagnostics.is_empty() {
        return Err(compiler.diagnostics);
    }
    let Some(root) = root else {
        return Err(vec![ConditionCompileDiagnostic {
            condition: definition.id,
            message: "condition program has no root instruction".to_owned(),
        }]);
    };
    let kernel_key = Sha256::digest(format!("{definition:?}").as_bytes()).into();
    Ok(CompiledConditionProgram {
        instructions: compiler.instructions,
        root,
        state_layout: ConditionStateLayout {
            keys: compiler.state_keys,
        },
        observation: compiler.observation,
        kernel_key,
    })
}

#[derive(Default)]
struct Compiler {
    instructions: Vec<CompiledConditionInstruction>,
    state_keys: Vec<ConditionStateKey>,
    observation: Vec<ConditionObservationDescriptor>,
    diagnostics: Vec<ConditionCompileDiagnostic>,
}

impl Compiler {
    fn state_slot(&mut self, condition: ConditionId) -> usize {
        let slot = self.state_keys.len();
        self.state_keys.push(ConditionStateKey { condition });
        slot
    }

    fn compile(&mut self, definition: &ConditionDefinition) -> Option<usize> {
        if !definition.enabled {
            return Some(self.push(definition, CompiledConditionInstruction::Constant(true)));
        }
        let instruction = match &definition.kind {
            ConditionKind::Constant(value) => CompiledConditionInstruction::Constant(*value),
            ConditionKind::InputValue(condition) => {
                let state_slot = self.state_slot(definition.id);
                CompiledConditionInstruction::InputValue {
                    input: condition.input.clone(),
                    projection: condition.projection,
                    comparator: condition.comparator,
                    expected: condition.expected.clone(),
                    expected_max: condition.expected_max.clone(),
                    behavior: condition.behavior,
                    state_slot,
                }
            }
            ConditionKind::InputNode(condition) => {
                if condition.provider.trim().is_empty() {
                    self.error(definition.id, "input-node condition has no provider");
                }
                let state_slot = self.state_slot(definition.id);
                CompiledConditionInstruction::InputNode {
                    provider: condition.provider.clone(),
                    node: condition.node.clone(),
                    projection: condition.projection,
                    comparator: condition.comparator,
                    expected: condition.expected.clone(),
                    expected_max: condition.expected_max.clone(),
                    behavior: condition.behavior,
                    state_slot,
                }
            }
            ConditionKind::Group { policy, children } => {
                if children.is_empty() {
                    self.error(definition.id, "condition group has no children");
                }
                let children = children.iter().filter_map(|child| self.compile(child)).collect();
                CompiledConditionInstruction::Group {
                    policy: *policy,
                    children,
                }
            }
            ConditionKind::Script(condition) => {
                if condition.script.trim().is_empty() {
                    self.error(definition.id, "script condition has no script identity");
                }
                let state_slot = self.state_slot(definition.id);
                CompiledConditionInstruction::Script {
                    script: condition.script.clone(),
                    behavior: condition.behavior,
                    state_slot,
                }
            }
        };
        Some(self.push(definition, instruction))
    }

    fn push(&mut self, definition: &ConditionDefinition, instruction: CompiledConditionInstruction) -> usize {
        let index = self.instructions.len();
        self.instructions.push(instruction);
        self.observation.push(ConditionObservationDescriptor {
            condition: definition.id,
            instruction: index,
            label: definition.label.clone(),
        });
        index
    }

    fn error(&mut self, condition: ConditionId, message: &str) {
        self.diagnostics.push(ConditionCompileDiagnostic {
            condition,
            message: message.to_owned(),
        });
    }
}

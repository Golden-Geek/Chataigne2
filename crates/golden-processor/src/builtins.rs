use golden_alchemist::{
    ANodeTypeId, AlchemistFormula, FormulaId, SingleNodeFormulaSpec, SingleNodeInputSpec, SingleNodeOutputSpec,
    build_single_node_formula,
};
use golden_model::EntityId;
use golden_values::{FiniteF64, Value, ValueTypeId};
use uuid::Uuid;

pub const ACTION_FORMULA_ID: FormulaId = FormulaId(EntityId::from_uuid(Uuid::from_u128(
    0xa11c_7100_0000_0000_0000_0000_0000_0001,
)));
pub const MAPPING_FORMULA_ID: FormulaId = FormulaId(EntityId::from_uuid(Uuid::from_u128(
    0xa11c_7100_0000_0000_0000_0000_0000_0002,
)));

pub struct BuiltinFormulaAssets;

impl BuiltinFormulaAssets {
    pub fn action() -> AlchemistFormula {
        build_single_node_formula(SingleNodeFormulaSpec {
            id: ACTION_FORMULA_ID,
            name: "Action".into(),
            description: "Built-in Action processor formula".into(),
            tags: vec!["built-in".into(), "processor".into()],
            node_type: ANodeTypeId("pass_through".into()),
            inputs: vec![float_input("value", "Value")],
            outputs: vec![float_output()],
        })
    }

    pub fn mapping() -> AlchemistFormula {
        build_single_node_formula(SingleNodeFormulaSpec {
            id: MAPPING_FORMULA_ID,
            name: "Mapping".into(),
            description: "Built-in Mapping processor formula".into(),
            tags: vec!["built-in".into(), "processor".into()],
            node_type: ANodeTypeId("condition_gate".into()),
            inputs: vec![
                SingleNodeInputSpec {
                    name: "condition".into(),
                    label: "Condition".into(),
                    value_type: ValueTypeId::new("bool").expect("built-in type is valid"),
                    default: Value::Bool(true),
                },
                float_input("value", "Value"),
            ],
            outputs: vec![float_output()],
        })
    }
}

fn float_input(name: &str, label: &str) -> SingleNodeInputSpec {
    SingleNodeInputSpec {
        name: name.into(),
        label: label.into(),
        value_type: ValueTypeId::new("float").expect("built-in type is valid"),
        default: Value::Float(FiniteF64::new(0.0).expect("zero is finite")),
    }
}

fn float_output() -> SingleNodeOutputSpec {
    SingleNodeOutputSpec {
        name: "output".into(),
        label: "Output".into(),
        value_type: ValueTypeId::new("float").expect("built-in type is valid"),
    }
}

use std::{collections::BTreeMap, sync::Arc};

use golden_graph::{
    GraphDocument, GraphId, GraphNode, GraphNodeId, GraphOperation, GraphPortId, GraphTransaction, PortRef,
};
use golden_model::Revision;
use golden_values::{FiniteF64, Value, ValueTypeId};

use super::*;

struct AddFixture {
    formula: AlchemistFormula,
    left: SurfaceItemId,
    right: SurfaceItemId,
    result: SurfaceItemId,
}

fn float(value: f64) -> Value {
    Value::Float(FiniteF64::new(value).unwrap())
}

fn port(name: &str) -> AlchemistPort {
    AlchemistPort {
        id: GraphPortId::new(),
        name: name.into(),
        value_type: ValueTypeId::new("float").unwrap(),
    }
}

fn add_fixture() -> AddFixture {
    let left_port = port("left");
    let right_port = port("right");
    let result_port = port("result");
    let node_id = GraphNodeId::new();
    let mut graph = GraphDocument::new(
        GraphId::new(),
        AlchemistGraphDomain,
        AlchemistGraphData {
            name: "add formula".into(),
        },
    );
    let mut transaction = GraphTransaction::new(Revision::ZERO);
    transaction.push(GraphOperation::InsertNode {
        node: GraphNode {
            id: node_id,
            data: AlchemistNode {
                node_type: ANodeTypeId("add_float".into()),
                inputs: vec![left_port.clone(), right_port.clone()],
                outputs: vec![result_port.clone()],
                config: BTreeMap::new(),
            },
        },
        presentation: None,
    });
    graph.apply(transaction).unwrap();

    let left = SurfaceItemId::new();
    let right = SurfaceItemId::new();
    let result = SurfaceItemId::new();
    AddFixture {
        formula: AlchemistFormula {
            id: FormulaId::new(),
            schema: FormulaSchema { version: 1 },
            graph,
            properties: Vec::new(),
            surface: FormulaSurface {
                inputs: vec![
                    SurfaceInput {
                        id: left,
                        label: "Left".into(),
                        target: PortRef {
                            node: node_id,
                            port: left_port.id,
                        },
                        value_type: left_port.value_type,
                        default: float(0.0),
                    },
                    SurfaceInput {
                        id: right,
                        label: "Right".into(),
                        target: PortRef {
                            node: node_id,
                            port: right_port.id,
                        },
                        value_type: right_port.value_type,
                        default: float(0.0),
                    },
                ],
                outputs: vec![SurfaceOutput {
                    id: result,
                    label: "Result".into(),
                    source: PortRef {
                        node: node_id,
                        port: result_port.id,
                    },
                    value_type: result_port.value_type,
                }],
            },
            managed_regions: Vec::new(),
            metadata: FormulaMetadata {
                name: "Add".into(),
                description: "Adds two finite floats".into(),
                tags: vec!["math".into()],
            },
            defaults: FormulaDefaults::default(),
        },
        left,
        right,
        result,
    }
}

#[test]
fn compiles_and_evaluates_a_formula_without_interpreting_the_graph() {
    let fixture = add_fixture();
    let registry = ANodeRegistry::with_builtins();
    let kernel = Arc::new(FormulaCompiler::new(&registry).compile(&fixture.formula).unwrap());
    let mut instance = FormulaInstance::new(kernel);
    instance.set_input(fixture.left, float(2.0)).unwrap();
    instance.set_input(fixture.right, float(3.5)).unwrap();

    let report = instance.evaluate(EvaluationOptions::default()).unwrap();

    assert_eq!(instance.output(fixture.result).unwrap(), &float(5.5));
    assert_eq!(report.executed_operations, 1);
    assert!(report.outputs_changed);
    assert!(report.observation.is_empty());
}

#[test]
fn compile_cache_shares_kernels_and_idle_instances_do_no_work() {
    let fixture = add_fixture();
    let registry = ANodeRegistry::with_builtins();
    let cache = FormulaCompileCache::default();
    let first = cache.compile(&fixture.formula, &registry).unwrap();
    let second = cache.compile(&fixture.formula, &registry).unwrap();
    assert!(Arc::ptr_eq(&first, &second));
    assert_eq!(cache.compilation_count(), 1);

    let mut instance = FormulaInstance::new(first);
    instance.evaluate(EvaluationOptions::default()).unwrap();
    let idle = instance.evaluate(EvaluationOptions::default()).unwrap();
    assert_eq!(idle.executed_operations, 0);
}

#[test]
fn observation_and_batching_are_opt_in_runtime_features() {
    let fixture = add_fixture();
    let registry = ANodeRegistry::with_builtins();
    let kernel = Arc::new(FormulaCompiler::new(&registry).compile(&fixture.formula).unwrap());
    let mut observed = FormulaInstance::new(Arc::clone(&kernel));
    let report = observed
        .evaluate(EvaluationOptions {
            capture_observation: true,
        })
        .unwrap();
    assert_eq!(report.observation.len(), 1);

    let mut instances = vec![FormulaInstance::new(Arc::clone(&kernel)), FormulaInstance::new(kernel)];
    let report = evaluate_batch(&mut instances, EvaluationOptions::default()).unwrap();
    assert_eq!(report.instances, 2);
    assert_eq!(report.executed_operations, 2);
}

#[test]
fn formula_file_round_trip_preserves_authored_contract() {
    let fixture = add_fixture();
    let json = encode_formula(&fixture.formula).unwrap();
    let decoded = decode_formula(&json).unwrap();

    assert_eq!(decoded.id, fixture.formula.id);
    assert_eq!(decoded.schema, fixture.formula.schema);
    assert_eq!(decoded.metadata, fixture.formula.metadata);
    assert_eq!(decoded.surface, fixture.formula.surface);
    assert_eq!(decoded.graph.nodes().len(), 1);
}

#[test]
fn catalog_protects_built_in_formulas() {
    let fixture = add_fixture();
    let id = fixture.formula.id;
    let formula = Arc::new(fixture.formula);
    let mut catalog = FormulaCatalog::default();
    catalog.insert(Arc::clone(&formula), true).unwrap();

    assert_eq!(
        catalog.insert(Arc::clone(&formula), false).unwrap_err(),
        FormulaCatalogError::Duplicate(id)
    );
    assert_eq!(catalog.replace(formula).unwrap_err(), FormulaCatalogError::ReadOnly(id));
}

use crate::test_support::TestGraph;

use chataigne_alchemist::{AlchemistFormula, FormulaContextContract, FormulaId, FormulaPropertySchema, FormulaSurface};

use crate::{Processor, ProcessorExecutionPolicy, ProcessorGroup, ProcessorManager};

fn formula() -> AlchemistFormula {
    AlchemistFormula {
        id: FormulaId::new("test"),
        version: 1,
        label: "Test".into(),
        description: None,
        tags: Vec::new(),
        graph: TestGraph::new().to_document().unwrap(),
        properties: FormulaPropertySchema::default(),
        surface: FormulaSurface::default(),
        context_contract: FormulaContextContract::default(),
        migrations: Vec::new(),
    }
}

#[test]
fn manager_orders_direct_and_grouped_processors_and_skips_disabled_scopes() {
    let formula = formula();
    let mut manager = ProcessorManager::new();
    let direct = Processor::from_formula("Direct", &formula);
    let direct_id = direct.id;
    manager.add_processor(direct).unwrap();

    let mut group = ProcessorGroup::new("Fixtures");
    group.execution_policy = ProcessorExecutionPolicy::ReverseInsertionOrder;
    let first = Processor::from_formula("First", &formula);
    let first_id = first.id;
    let second = Processor::from_formula("Second", &formula);
    let second_id = second.id;
    group.add_processor(first).unwrap();
    group.add_processor(second).unwrap();
    let group_id = manager.add_group(group).unwrap();

    assert_eq!(manager.active_processor_ids(), vec![direct_id, second_id, first_id]);

    manager.groups[&group_id].enabled = false;
    assert_eq!(manager.active_processor_ids(), vec![direct_id]);
}

use std::collections::HashSet;

use golden_alchemist::{FormulaFamily, SurfaceItemId};

use crate::builtin_formulas;

#[test]
fn builtins_are_only_action_and_mapping_formulas() {
    let formulas = builtin_formulas();
    let ids: HashSet<_> = formulas.iter().map(|formula| formula.id.clone()).collect();
    let families: HashSet<_> = formulas.iter().map(|formula| formula.family).collect();

    assert_eq!(formulas.len(), 2);
    assert_eq!(ids.len(), 2);
    assert_eq!(families, HashSet::from([FormulaFamily::Action, FormulaFamily::Mapping]));
    for formula in formulas {
        assert_eq!(formula.version, 1);
        assert!(!formula.graph.nodes.is_empty());
        assert!(!formula.surface.sections.is_empty());
        let instance = formula.instantiate();
        assert_eq!(instance.formula_ref.id, formula.id);
        assert_eq!(instance.formula_ref.version, formula.version);
        assert!(instance.overrides.values.is_empty());
        assert!(
            instance
                .surface_bindings
                .bindings
                .contains_key(&SurfaceItemId::new("target"))
                || formula.family == FormulaFamily::CustomUser
        );
    }
}

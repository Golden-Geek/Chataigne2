use std::collections::HashSet;

use crate::builtin_processor_models;

#[test]
fn builtin_models_are_versioned_graph_templates_with_exposed_surfaces() {
    let models = builtin_processor_models();
    let ids: HashSet<_> = models.iter().map(|model| model.id.clone()).collect();

    assert_eq!(models.len(), 7);
    assert_eq!(ids.len(), 7);
    for model in models {
        assert_eq!(model.version, 1);
        assert!(!model.graph_template.nodes.is_empty());
        assert!(!model.exposed_surface.params.is_empty());
        let instance = model.instantiate();
        assert_eq!(instance.model_id, model.id);
        assert!(instance.overrides.is_empty());
    }
}

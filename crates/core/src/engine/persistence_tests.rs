use super::ProjectNodeMeta;
use crate::node::{DeclId, NodeUserPermissions, NodeUuid, PresentationHint, SemanticsHint};
use serde_json::json;

fn project_meta(label: &str) -> ProjectNodeMeta {
    ProjectNodeMeta {
        decl_id: Some(DeclId(label.to_string())),
        short_name: Some(label.to_string()),
        enabled: Some(true),
        can_be_disabled: Some(true),
        label: Some(label.to_string()),
        description: Some(None),
        declared_description_key: Some(None),
        declared_description: Some(None),
        tags: Some(Vec::new()),
        user_permissions: Some(NodeUserPermissions::default()),
        semantics: Some(SemanticsHint::default()),
        presentation: Some(PresentationHint::default()),
    }
}

#[test]
fn project_decode_defaults_nested_inspector_visibility_on() {
    for node_type in ["regular", "script", "user_context"] {
        let meta = project_meta(node_type).into_runtime(NodeUuid::default());

        assert!(
            meta.presentation.show_in_nested_inspector,
            "{node_type} should default to visible in nested inspectors after load"
        );
    }
}

#[test]
fn project_meta_delta_preserves_explicit_optional_clears() {
    let baseline = ProjectNodeMeta {
        description: Some(Some("Default description".to_string())),
        declared_description_key: Some(Some("node::field".to_string())),
        declared_description: Some(Some("Declared description".to_string())),
        ..project_meta("node")
    };
    let current = ProjectNodeMeta {
        description: Some(None),
        declared_description_key: Some(None),
        declared_description: Some(None),
        ..baseline.clone()
    };

    let delta = current.delta_against(&baseline);

    assert_eq!(delta.description, Some(None));
    assert_eq!(delta.declared_description_key, Some(None));
    assert_eq!(delta.declared_description, Some(None));
    assert_eq!(
        serde_json::to_value(delta).expect("metadata delta should serialize"),
        json!({
            "description": null,
            "declared_description_key": null,
            "declared_description": null
        })
    );
}

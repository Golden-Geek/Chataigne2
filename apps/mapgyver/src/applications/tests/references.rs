use super::*;

#[test]
fn undo_and_reload_preserve_data_without_reusing_definition_observation_fences() {
    let mut engine = fixture();
    let application = create(&mut engine);
    let before = crate::applications::inspect(&engine, application).unwrap();
    let authored = to_sparse_project_json_pretty(&engine).unwrap();
    app_edit(&mut engine, ApplicationEdit::Remove { application });
    assert!(engine.undo().unwrap());
    let restored = crate::applications::inspect(&engine, application).unwrap();
    assert_eq!(restored.id, before.id);
    assert_eq!(restored.definition, before.definition);
    assert_ne!(
        (restored.definition_instance, restored.revision),
        (before.definition_instance, before.revision)
    );
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), authored);
    let loaded = from_sparse_project_json::<AppNode>(&authored).unwrap();
    let reloaded = crate::applications::inspect(&loaded, application).unwrap();
    assert_eq!(reloaded.id, before.id);
    assert_eq!(reloaded.definition, before.definition);
    assert_ne!(reloaded.definition_instance, before.definition_instance);
}

#[test]
fn bounded_reference_lookup_resolves_names_and_missing_definitions_without_authored_or_runtime_effects() {
    let mut engine = fixture();
    let application = create(&mut engine);
    let missing = ApplicationId(golden_core::model::NodeUuid(uuid::Uuid::new_v4()));
    app_edit(
        &mut engine,
        ApplicationEdit::Rename {
            application,
            label: "Shared renderer".into(),
        },
    );
    let history = engine.undo_len();
    let authored = to_sparse_project_json_pretty(&engine).unwrap();
    let MediaEditResult::ApplicationReferences { references } = app_edit(
        &mut engine,
        ApplicationEdit::InspectReferences {
            applications: vec![missing, application],
        },
    ) else {
        panic!()
    };
    assert_eq!(references.len(), 2);
    assert_eq!(references[0].application, missing);
    assert!(references[0].summary.is_none());
    assert_eq!(references[1].summary.as_ref().unwrap().label, "Shared renderer");
    assert!(!references[1].summary.as_ref().unwrap().configured);
    assert_eq!(engine.undo_len(), history);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), authored);
    let MediaEditResult::ApplicationRuntime { snapshot } = app_edit(&mut engine, ApplicationEdit::InspectRuntime {})
    else {
        panic!()
    };
    assert!(snapshot.applications.is_empty());
    app_edit(&mut engine, ApplicationEdit::Remove { application });
    let MediaEditResult::ApplicationReferences { references } = app_edit(
        &mut engine,
        ApplicationEdit::InspectReferences {
            applications: vec![application],
        },
    ) else {
        panic!()
    };
    assert!(references[0].summary.is_none());
}

#[test]
fn reference_lookup_rejects_duplicate_or_oversized_queries_without_mutation() {
    let mut engine = fixture();
    let application = create(&mut engine);
    let history = engine.undo_len();
    for applications in [
        vec![application, application],
        (0..65)
            .map(|_| ApplicationId(golden_core::model::NodeUuid(uuid::Uuid::new_v4())))
            .collect(),
    ] {
        let ack = invoke(
            &mut engine,
            MediaEdit::Applications {
                edit: ApplicationEdit::InspectReferences { applications },
            },
        );
        assert!(!ack.success);
        assert_eq!(engine.undo_len(), history);
    }
    let MediaEditResult::ApplicationReferences { references } =
        app_edit(&mut engine, ApplicationEdit::InspectReferences { applications: vec![] })
    else {
        panic!()
    };
    assert!(references.is_empty());
}

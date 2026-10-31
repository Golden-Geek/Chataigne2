use super::*;

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

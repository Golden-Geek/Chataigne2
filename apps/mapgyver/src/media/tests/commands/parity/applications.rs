use super::*;
use crate::applications::protocol::*;
use crate::matrix::protocol::{ColumnDefinition, MatrixEdit};

fn application(engine: &mut AppEngine, edit: ApplicationEdit) -> MediaEditResult {
    apply(engine, MediaEdit::Applications { edit })
}

struct Scene {
    document: String,
    primary: ApplicationId,
    secondary: ApplicationId,
    missing: ApplicationId,
    matrix: ChannelReference,
    column: crate::matrix::protocol::ColumnId,
    definition: ApplicationDefinition,
}

fn scene() -> Scene {
    let mut engine = fixture();
    let [primary, secondary, missing] = ["Parity Application", "Disabled Application", "Removed"].map(|label| {
        let MediaEditResult::ApplicationCreated { application: id } = application(
            &mut engine,
            ApplicationEdit::Create {
                label: Some(label.into()),
            },
        ) else {
            panic!()
        };
        id
    });
    let definition = ApplicationDefinition {
        executable: Some(std::env::current_exe().unwrap().to_string_lossy().into()),
        arguments: vec!["literal & value".into(), "two words".into()],
        directory: Some(std::env::current_dir().unwrap().to_string_lossy().into()),
        environment: Some(vec![("MAPGYVER_PARITY".into(), "literal = value".into())]),
        window: ApplicationWindow::Normal,
        readiness: ApplicationReadiness::Spout {
            sender: "Parity Sender".into(),
            first_frame: true,
            timeout_ms: 1500,
        },
        ..Default::default()
    };
    application(
        &mut engine,
        ApplicationEdit::Configure {
            application: primary,
            definition: definition.clone(),
        },
    );
    application(
        &mut engine,
        ApplicationEdit::SetEnabled {
            application: secondary,
            enabled: false,
        },
    );
    application(&mut engine, ApplicationEdit::Remove { application: missing });
    let matrix = create(&mut engine, MediaKind::Matrix);
    let MediaEditResult::MatrixCreated { columns, .. } = apply(
        &mut engine,
        MediaEdit::Matrix {
            media: matrix.media,
            edit: MatrixEdit::AddColumns { count: 3 },
        },
    ) else {
        panic!()
    };
    let column = columns[1];
    apply(
        &mut engine,
        MediaEdit::Matrix {
            media: matrix.media,
            edit: MatrixEdit::ConfigureColumn {
                column,
                definition: ColumnDefinition {
                    applications: vec![ColumnApplication {
                        application: primary,
                        pre: 1,
                        post: 1,
                    }],
                    ..Default::default()
                },
            },
        },
    );
    Scene {
        document: to_sparse_project_json_pretty(&engine).unwrap(),
        primary,
        secondary,
        missing,
        matrix,
        column,
        definition,
    }
}

#[test]
fn application_authoring_and_atomic_rejection_match_every_public_caller() {
    let scene = scene();
    let mut configured = scene.definition.clone();
    configured.readiness = ApplicationReadiness::ProcessStarted;
    configured.window = ApplicationWindow::Hidden;
    configured.restart.enabled = false;
    configured.stop = ApplicationStop {
        graceful_ms: 0,
        retirement_ms: 1000,
    };
    let mut invalid_argument = configured.clone();
    invalid_argument.arguments.push("invalid\0argument".into());
    let mut invalid_sender = configured.clone();
    invalid_sender.readiness = ApplicationReadiness::Spout {
        sender: String::new(),
        first_frame: false,
        timeout_ms: 0,
    };
    let mut invalid_stop = configured.clone();
    invalid_stop.stop.retirement_ms = 0;
    let mut invalid_restart = configured.clone();
    invalid_restart.restart.maximum_ms = 1;
    let cases = [
        (
            ApplicationEdit::Configure {
                application: scene.primary,
                definition: configured.clone(),
            },
            true,
        ),
        (
            ApplicationEdit::Configure {
                application: scene.primary,
                definition: scene.definition.clone(),
            },
            true,
        ),
        (
            ApplicationEdit::SetEnabled {
                application: scene.primary,
                enabled: false,
            },
            true,
        ),
        (
            ApplicationEdit::SetEnabled {
                application: scene.secondary,
                enabled: true,
            },
            true,
        ),
        (
            ApplicationEdit::SetEnabled {
                application: scene.secondary,
                enabled: false,
            },
            true,
        ),
        (
            ApplicationEdit::Rename {
                application: scene.primary,
                label: "Renamed application".into(),
            },
            true,
        ),
        (
            ApplicationEdit::Remove {
                application: scene.primary,
            },
            true,
        ),
        (
            ApplicationEdit::Configure {
                application: scene.primary,
                definition: invalid_argument,
            },
            false,
        ),
        (
            ApplicationEdit::Configure {
                application: scene.primary,
                definition: invalid_sender,
            },
            false,
        ),
        (
            ApplicationEdit::Configure {
                application: scene.primary,
                definition: invalid_stop,
            },
            false,
        ),
        (
            ApplicationEdit::Configure {
                application: scene.primary,
                definition: invalid_restart,
            },
            false,
        ),
        (
            ApplicationEdit::Configure {
                application: scene.missing,
                definition: configured,
            },
            false,
        ),
        (
            ApplicationEdit::Remove {
                application: scene.missing,
            },
            false,
        ),
        (
            ApplicationEdit::Rename {
                application: scene.primary,
                label: String::new(),
            },
            false,
        ),
    ];
    for (edit, success) in cases {
        let mut canonical = None;
        for caller in CALLERS {
            let mut engine = loaded(&scene.document);
            let reply = run(&mut engine, caller, MediaEdit::Applications { edit: edit.clone() });
            assert_eq!(reply.is_some(), success, "{caller:?}: {edit:?}");
            let actual = (reply, authored(&engine));
            if let Some(expected) = &canonical {
                assert_eq!(&actual, expected, "{caller:?}: {edit:?}");
            } else {
                canonical = Some(actual);
            }
        }
    }
}

#[test]
fn application_creation_and_copy_return_committed_definitions_through_all_callers() {
    let scene = scene();
    for caller in CALLERS {
        for edit in [
            ApplicationEdit::Create { label: None },
            ApplicationEdit::Create {
                label: Some("Parity Application".into()),
            },
            ApplicationEdit::Duplicate {
                application: scene.primary,
            },
            ApplicationEdit::Duplicate {
                application: scene.secondary,
            },
        ] {
            let mut engine = loaded(&scene.document);
            let reply = run(&mut engine, caller, MediaEdit::Applications { edit: edit.clone() }).unwrap();
            let MediaEditResult::ApplicationCreated { application: created } = serde_json::from_value(reply).unwrap()
            else {
                panic!()
            };
            assert_ne!(created, scene.primary);
            assert_ne!(created, scene.secondary);
            let snapshot = crate::applications::inspect(&engine, created).unwrap();
            if let ApplicationEdit::Duplicate { application } = edit {
                let original = crate::applications::inspect(&engine, application).unwrap();
                assert_eq!(snapshot.definition, original.definition);
                assert_eq!(snapshot.enablement, original.enablement);
                assert_ne!(snapshot.definition_instance, original.definition_instance);
            }
            let names = authored(&engine)
                .3
                .into_iter()
                .map(|entry| entry.1)
                .collect::<std::collections::HashSet<_>>();
            assert_eq!(names.len(), 3, "backend allocates unique committed labels");
        }
    }
}

#[test]
fn application_observation_bounds_and_stale_callback_tickets_match_all_callers() {
    let scene = scene();
    let queries = [
        (
            ApplicationEdit::Inspect {
                application: scene.primary,
            },
            true,
        ),
        (
            ApplicationEdit::Inspect {
                application: scene.missing,
            },
            false,
        ),
        (
            ApplicationEdit::InspectCatalog {
                search: String::new(),
                start: 0,
                count: 128,
            },
            true,
        ),
        (
            ApplicationEdit::InspectCatalog {
                search: "APPLICATION".into(),
                start: 1,
                count: 1,
            },
            true,
        ),
        (
            ApplicationEdit::InspectCatalog {
                search: "absent".into(),
                start: 0,
                count: 1,
            },
            true,
        ),
        (
            ApplicationEdit::InspectCatalog {
                search: String::new(),
                start: 0,
                count: 0,
            },
            false,
        ),
        (
            ApplicationEdit::InspectCatalog {
                search: String::new(),
                start: 0,
                count: 129,
            },
            false,
        ),
        (
            ApplicationEdit::InspectCatalog {
                search: "a".repeat(257),
                start: 0,
                count: 1,
            },
            false,
        ),
        (
            ApplicationEdit::InspectReferences {
                applications: vec![scene.primary, scene.secondary, scene.missing],
            },
            true,
        ),
        (
            ApplicationEdit::InspectReferences {
                applications: vec![scene.primary; 2],
            },
            false,
        ),
        (
            ApplicationEdit::InspectReferences {
                applications: (0..65)
                    .map(|_| ApplicationId(golden_core::model::NodeUuid(uuid::Uuid::new_v4())))
                    .collect(),
            },
            false,
        ),
        (ApplicationEdit::InspectRuntime {}, true),
        (ApplicationEdit::InspectCallbacks {}, true),
        (
            ApplicationEdit::DispatchCallbacks {
                ticket: ApplicationCallbackTicket {
                    project: uuid::Uuid::nil(),
                    batch: uuid::Uuid::nil(),
                },
            },
            false,
        ),
    ];
    for (edit, success) in queries {
        let mut canonical = None;
        for caller in CALLERS {
            let mut engine = loaded(&scene.document);
            let reply = run(&mut engine, caller, MediaEdit::Applications { edit: edit.clone() });
            assert_eq!(reply.is_some(), success, "{caller:?}: {edit:?}");
            let Some(reply) = reply else { continue };
            let semantic = match serde_json::from_value::<MediaEditResult>(reply.clone()).unwrap() {
                MediaEditResult::Application { snapshot } => {
                    assert_eq!(snapshot.id, scene.primary);
                    assert_eq!(snapshot.definition, scene.definition);
                    assert!(!snapshot.definition_instance.is_nil());
                    serde_json::json!([snapshot.id, snapshot.label, snapshot.enablement, snapshot.definition])
                }
                MediaEditResult::ApplicationRuntime { snapshot } => {
                    assert!(snapshot.applications.is_empty(), "observing cannot launch a process");
                    assert_eq!(snapshot.callbacks.pending, 0);
                    serde_json::json!([snapshot.supervision, snapshot.callbacks, snapshot.applications])
                }
                MediaEditResult::Applications { page } => {
                    let ApplicationEdit::InspectCatalog { search, start, count } = &edit else {
                        panic!()
                    };
                    assert_eq!(page.start, *start);
                    assert!(page.applications.len() <= usize::from(*count));
                    let expected = if search == "absent" {
                        vec![]
                    } else {
                        vec![scene.primary, scene.secondary]
                    };
                    assert_eq!(page.total as usize, expected.len());
                    assert_eq!(
                        page.applications.iter().map(|entry| entry.id).collect::<Vec<_>>(),
                        expected
                            .into_iter()
                            .skip(*start as usize)
                            .take(usize::from(*count))
                            .collect::<Vec<_>>()
                    );
                    reply
                }
                MediaEditResult::ApplicationReferences { references } => {
                    assert_eq!(
                        references.iter().map(|entry| entry.application).collect::<Vec<_>>(),
                        vec![scene.primary, scene.secondary, scene.missing]
                    );
                    let primary = references[0].summary.as_ref().unwrap();
                    assert!(primary.enabled && primary.configured);
                    let secondary = references[1].summary.as_ref().unwrap();
                    assert!(!secondary.enabled && !secondary.configured);
                    assert!(references[2].summary.is_none());
                    reply
                }
                MediaEditResult::ApplicationCallbacks { state } => {
                    assert_eq!(state, ApplicationCallbackState::default());
                    reply
                }
                _ => reply,
            };
            if let Some(expected) = &canonical {
                assert_eq!(&semantic, expected, "{caller:?}: {edit:?}");
            } else {
                canonical = Some(semantic);
            }
        }
    }
}

#[test]
fn column_application_associations_share_committed_history_and_atomic_rejection() {
    let scene = scene();
    for (applications, success) in [
        (
            vec![
                ColumnApplication {
                    application: scene.secondary,
                    pre: 2,
                    post: 0,
                },
                ColumnApplication {
                    application: scene.primary,
                    pre: 0,
                    post: 2,
                },
            ],
            true,
        ),
        (vec![], true),
        (
            vec![
                ColumnApplication {
                    application: scene.primary,
                    pre: 1,
                    post: 1
                };
                2
            ],
            false,
        ),
        (
            vec![ColumnApplication {
                application: scene.missing,
                pre: 0,
                post: 0,
            }],
            false,
        ),
    ] {
        let mut canonical = None;
        for caller in CALLERS {
            let mut engine = loaded(&scene.document);
            let reply = run(
                &mut engine,
                caller,
                MediaEdit::Matrix {
                    media: scene.matrix.media,
                    edit: MatrixEdit::ConfigureColumn {
                        column: scene.column,
                        definition: ColumnDefinition {
                            applications: applications.clone(),
                            ..Default::default()
                        },
                    },
                },
            );
            assert_eq!(reply.is_some(), success, "{caller:?}: {applications:?}");
            let actual = (reply, authored(&engine));
            if let Some(expected) = &canonical {
                assert_eq!(&actual, expected, "{caller:?}");
            } else {
                canonical = Some(actual);
            }
        }
    }
}

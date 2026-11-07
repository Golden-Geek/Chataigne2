use super::*;
use crate::applications::protocol::*;
use crate::matrix::protocol::*;

#[test]
fn named_column_cell_and_unload_calls_preserve_anchor_policy_without_authored_history() {
    let mut baseline = fixture();
    let matrix = create(&mut baseline, MediaKind::Matrix);
    let MediaEditResult::MatrixCreated { items, .. } = apply(
        &mut baseline,
        MediaEdit::Matrix {
            media: matrix.media,
            edit: MatrixEdit::AddLayers { parent: None, count: 1 },
        },
    ) else {
        panic!()
    };
    let MatrixItemId::Layer { id: layer } = items[0] else {
        panic!()
    };
    let MediaEditResult::MatrixCreated { columns, .. } = apply(
        &mut baseline,
        MediaEdit::Matrix {
            media: matrix.media,
            edit: MatrixEdit::AddColumns { parent: None, count: 2 },
        },
    ) else {
        panic!()
    };
    apply(
        &mut baseline,
        MediaEdit::Matrix {
            media: matrix.media,
            edit: MatrixEdit::ConfigureCell {
                layer,
                column: columns[1],
                settings: CellSettings::default(),
            },
        },
    );
    let document = to_sparse_project_json_pretty(&baseline).unwrap();
    for caller in CALLERS {
        for operation in [
            MatrixEdit::LaunchColumn {
                column: columns[0],
                mode: LaunchMode::Cut,
            },
            MatrixEdit::LaunchCell {
                layer,
                column: columns[1],
                mode: LaunchMode::Cut,
            },
            MatrixEdit::ClearPlayback,
        ] {
            let mut engine = loaded(&document);
            apply(
                &mut engine,
                MediaEdit::Matrix {
                    media: matrix.media,
                    edit: MatrixEdit::LaunchColumn {
                        column: columns[0],
                        mode: LaunchMode::Cut,
                    },
                },
            );
            let individual = matches!(operation, MatrixEdit::LaunchCell { .. });
            let unloaded = matches!(operation, MatrixEdit::ClearPlayback);
            let reply = run(
                &mut engine,
                caller,
                MediaEdit::Matrix {
                    media: matrix.media,
                    edit: operation,
                },
            )
            .unwrap();
            assert!(matches!(
                serde_json::from_value::<MediaEditResult>(reply).unwrap(),
                MediaEditResult::MatrixControlInvoked { .. }
            ));
            let MediaEditResult::MatrixPlayback { snapshot } =
                crate::matrix::apply(&mut engine, matrix.media, MatrixEdit::InspectPlayback).unwrap()
            else {
                panic!()
            };
            assert_eq!(snapshot.desired_column, (!unloaded).then_some(columns[0]));
            if individual {
                assert!(
                    snapshot.layers.iter().any(|request| request.layer == layer
                        && request.column == Some(columns[1])
                        && request.individual)
                );
            }
        }
    }
}

#[test]
fn owned_application_configuration_inspection_and_rejection_match_every_public_caller() {
    let mut engine = fixture();
    let matrix = create(&mut engine, MediaKind::Matrix);
    let MediaEditResult::MatrixCreated { columns, .. } = apply(
        &mut engine,
        MediaEdit::Matrix {
            media: matrix.media,
            edit: MatrixEdit::AddColumns { parent: None, count: 2 },
        },
    ) else {
        panic!()
    };
    let primary = ApplicationId(columns[0].0);
    let missing = ApplicationId(golden_core::node::NodeUuid::nil());
    let definition = ApplicationDefinition {
        executable: Some("C:\\mapgyver-fixture-missing\\never.exe".into()),
        arguments: vec!["literal & value".into(), "two words".into()],
        environment: Some(vec![("MAPGYVER_PARITY".into(), "literal = value".into())]),
        ..Default::default()
    };
    let mut invalid = definition.clone();
    invalid.arguments.push("invalid\0argument".into());
    let mut invalid_stop = definition.clone();
    invalid_stop.stop.retirement_ms = 0;
    let mut invalid_sender = definition.clone();
    invalid_sender.readiness = ApplicationReadiness::Spout {
        sender: String::new(),
        first_frame: false,
        timeout_ms: 0,
    };
    let mut invalid_restart = definition.clone();
    invalid_restart.restart.maximum_ms = 1;
    let document = to_sparse_project_json_pretty(&engine).unwrap();
    let cases = vec![
        (
            MediaEdit::Applications {
                edit: ApplicationEdit::SetFile {
                    application: primary,
                    executable: Some(std::env::current_exe().unwrap().to_string_lossy().into()),
                },
            },
            true,
        ),
        (
            MediaEdit::Applications {
                edit: ApplicationEdit::SetFile {
                    application: primary,
                    executable: None,
                },
            },
            true,
        ),
        (
            MediaEdit::Applications {
                edit: ApplicationEdit::SetFile {
                    application: primary,
                    executable: Some("relative.exe".into()),
                },
            },
            false,
        ),
        (
            MediaEdit::Applications {
                edit: ApplicationEdit::Configure {
                    application: primary,
                    definition: invalid_sender,
                },
            },
            false,
        ),
        (
            MediaEdit::Applications {
                edit: ApplicationEdit::Configure {
                    application: primary,
                    definition: invalid_restart,
                },
            },
            false,
        ),
        (
            MediaEdit::Applications {
                edit: ApplicationEdit::DispatchCallbacks {
                    ticket: ApplicationCallbackTicket {
                        project: uuid::Uuid::nil(),
                        batch: uuid::Uuid::nil(),
                    },
                },
            },
            false,
        ),
        (
            MediaEdit::Applications {
                edit: ApplicationEdit::Configure {
                    application: primary,
                    definition: definition.clone(),
                },
            },
            true,
        ),
        (
            MediaEdit::Applications {
                edit: ApplicationEdit::Configure {
                    application: primary,
                    definition: invalid,
                },
            },
            false,
        ),
        (
            MediaEdit::Applications {
                edit: ApplicationEdit::Configure {
                    application: primary,
                    definition: invalid_stop,
                },
            },
            false,
        ),
        (
            MediaEdit::Applications {
                edit: ApplicationEdit::Configure {
                    application: missing,
                    definition: definition.clone(),
                },
            },
            false,
        ),
        (
            MediaEdit::Applications {
                edit: ApplicationEdit::Inspect { application: primary },
            },
            true,
        ),
        (
            MediaEdit::Applications {
                edit: ApplicationEdit::Inspect { application: missing },
            },
            true,
        ),
        (
            MediaEdit::Applications {
                edit: ApplicationEdit::InspectRuntime {},
            },
            true,
        ),
        (
            MediaEdit::Applications {
                edit: ApplicationEdit::InspectCallbacks {},
            },
            true,
        ),
        (
            MediaEdit::Matrix {
                media: matrix.media,
                edit: MatrixEdit::ConfigureColumnApplication {
                    column: columns[0],
                    definition: definition.clone(),
                    pre: u16::MAX,
                    post: 1,
                },
            },
            true,
        ),
        (
            MediaEdit::Matrix {
                media: matrix.media,
                edit: MatrixEdit::ConfigureColumn {
                    column: columns[0],
                    definition: ColumnDefinition {
                        transition: Some(TransitionDefinition {
                            duration_ms: u32::MAX,
                            curve: TransitionCurve::SmoothStep,
                        }),
                    },
                },
            },
            true,
        ),
    ];
    for (operation, success) in cases {
        let mut expected = None;
        for caller in CALLERS {
            let mut engine = loaded(&document);
            let mut reply = run(&mut engine, caller, operation.clone());
            if let Some(reply) = &mut reply {
                if reply["kind"] == "Application" {
                    reply["snapshot"].as_object_mut().unwrap().remove("definition_instance");
                }
                if reply["kind"] == "ApplicationRuntime" {
                    reply["snapshot"].as_object_mut().unwrap().remove("project");
                }
            }
            assert_eq!(reply.is_some(), success, "{caller:?}: {operation:?}");
            let state = authored(&engine);
            if let Some((expected_reply, expected_state)) = &expected {
                assert_eq!(&reply, expected_reply, "{caller:?}");
                assert_eq!(&state, expected_state, "{caller:?}");
            } else {
                expected = Some((reply, state));
            }
        }
    }
}

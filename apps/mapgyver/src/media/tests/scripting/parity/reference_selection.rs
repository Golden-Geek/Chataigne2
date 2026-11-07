use super::*;

fn selected(engine: &AppEngine, output: ChannelReference) -> Option<ChannelReference> {
    let ChannelSource::Reference { target } = channel(engine, output).unwrap().definition.source else {
        panic!("Reference output required")
    };
    target
}

#[test]
fn reference_channel_selection_rejection_and_recovery_match_every_caller() {
    let mut engine = fixture();
    let first = create(&mut engine, MediaKind::SolidColor);
    let MediaEditResult::Created { reference: second } = apply(
        &mut engine,
        MediaEdit::AddChannel {
            media: first.media,
            label: None,
        },
    ) else {
        panic!()
    };
    let foreign = create(&mut engine, MediaKind::SolidColor);
    let MediaEditResult::ReferenceCreated { media } = apply(
        &mut engine,
        MediaEdit::CreateReference {
            target: Some(first.media),
            label: None,
        },
    ) else {
        panic!()
    };
    let output = crate::media::resolve_source_node(&engine, media.0).unwrap();
    assert_eq!(selected(&engine, output), Some(first));
    apply(
        &mut engine,
        MediaEdit::SelectReferenceChannel {
            media,
            channel: Some(second.channel),
        },
    );
    let pinned = to_sparse_project_json_pretty(&engine).unwrap();
    apply(&mut engine, MediaEdit::RemoveChannel { reference: second });
    let missing = to_sparse_project_json_pretty(&engine).unwrap();
    assert_eq!(selected(&engine, output), Some(second));
    assert!(engine.undo().unwrap());
    assert_eq!(selected(&engine, output), Some(second));
    apply(&mut engine, MediaEdit::ConfigureReference { media, target: None });
    let empty = to_sparse_project_json_pretty(&engine).unwrap();
    for (baseline, operation, applied, expected) in [
        (
            &pinned,
            MediaEdit::SelectReferenceChannel {
                media,
                channel: Some(first.channel),
            },
            true,
            Some(first),
        ),
        (
            &pinned,
            MediaEdit::SelectReferenceChannel {
                media,
                channel: Some(foreign.channel),
            },
            false,
            Some(second),
        ),
        (
            &pinned,
            MediaEdit::SelectReferenceChannel { media, channel: None },
            true,
            Some(first),
        ),
        (
            &pinned,
            MediaEdit::ConfigureReference {
                media,
                target: Some(first.media),
            },
            true,
            Some(second),
        ),
        (
            &pinned,
            MediaEdit::ConfigureReference {
                media,
                target: Some(foreign.media),
            },
            true,
            Some(foreign),
        ),
        (
            &pinned,
            MediaEdit::ConfigureReference { media, target: None },
            true,
            None,
        ),
        (
            &missing,
            MediaEdit::SelectReferenceChannel {
                media,
                channel: Some(second.channel),
            },
            true,
            Some(second),
        ),
        (
            &missing,
            MediaEdit::SelectReferenceChannel { media, channel: None },
            true,
            Some(first),
        ),
        (
            &empty,
            MediaEdit::SelectReferenceChannel {
                media,
                channel: Some(first.channel),
            },
            false,
            None,
        ),
    ] {
        let mut canonical = None;
        for caller in CALLERS {
            let mut engine = loaded(baseline);
            let result = run(&mut engine, caller, operation.clone());
            assert_eq!(result.is_some(), applied, "{caller:?}: {operation:?}");
            assert_eq!(selected(&engine, output), expected);
            let actual = (result, authored(&engine));
            if let Some(expected) = &canonical {
                assert_eq!(&actual, expected, "{caller:?}: {operation:?}");
            } else {
                canonical = Some(actual);
            }
            let saved = to_sparse_project_json_pretty(&engine).unwrap();
            let restored = loaded(&saved);
            crate::media::validate_project(&restored).unwrap();
            assert_eq!(crate::media::resolve_source_node(&restored, media.0).unwrap(), output);
            assert_eq!(selected(&restored, output), expected);
        }
    }
}

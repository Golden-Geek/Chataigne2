use super::*;

#[test]
fn video_source_status_lifetime_absence_matches_every_public_caller_without_history() {
    let mut engine = fixture();
    let reference = create(&mut engine, MediaKind::Video);
    let document = to_sparse_project_json_pretty(&engine).unwrap();
    apply(&mut engine, MediaEdit::RemoveMedia { media: reference.media });
    let retired = to_sparse_project_json_pretty(&engine).unwrap();
    for caller in CALLERS {
        for (document, absent) in [(&document, false), (&retired, true)] {
            let mut engine = loaded(document);
            let result = run(&mut engine, caller, MediaEdit::InspectVideoSource { reference }).unwrap();
            if absent {
                assert!(result["status"].is_null(), "{caller:?}");
            } else {
                assert_eq!(result["status"]["state"], "Unconfigured");
            }
        }
    }
}

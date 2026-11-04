use super::*;

#[test]
fn picture_file_edits_binding_queries_and_rejections_match_every_public_caller() {
    let baseline = baseline();
    let reference = baseline.references[1][0];
    for operation in [
        MediaEdit::ConfigurePictureFile {
            reference,
            file: "images/updated.png".into(),
        },
        MediaEdit::ConfigurePictureFile {
            reference,
            file: String::new(),
        },
        MediaEdit::ConfigurePictureFile {
            reference,
            file: "../invalid.png".into(),
        },
        MediaEdit::InspectPictureFile { reference },
    ] {
        let mut expected = None;
        for caller in CALLERS {
            let mut engine = loaded(&baseline.json);
            let result = run(&mut engine, caller, operation.clone());
            let actual = (result, authored(&engine));
            if let Some(expected) = &expected {
                assert_eq!(&actual, expected, "{caller:?}");
            } else {
                expected = Some(actual);
            }
        }
    }
}

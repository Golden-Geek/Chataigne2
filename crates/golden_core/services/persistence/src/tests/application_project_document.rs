use golden_model::{NodeUuid, UserNodeRole};

use crate::{
    ApplicationProjectDocument, PROJECT_FILE_VERSION, ProjectDocument, ProjectDocumentCodecError, ProjectNodeRecord,
    decode_application_project_document, encode_application_project_document, encode_project_document,
};

fn fixture() -> ApplicationProjectDocument {
    ApplicationProjectDocument {
        application_id: "fixture-app".into(),
        project: ProjectDocument {
            version: PROJECT_FILE_VERSION.into(),
            ui_state: Some(serde_json::json!({"panel": "graph"})),
            root: ProjectNodeRecord {
                uuid: NodeUuid::nil(),
                node_type: "folder".into(),
                user_role: UserNodeRole::Regular,
                meta: serde_json::Value::Null,
                data: None,
                children: Vec::new(),
            },
        },
    }
}

#[test]
fn application_envelope_round_trips_without_changing_the_graph_format() {
    let document = fixture();
    let encoded = encode_application_project_document(&document, "fixture-app").unwrap();
    assert_eq!(
        decode_application_project_document::<serde_json::Value>(&encoded, "fixture-app").unwrap(),
        document
    );
    let wire: serde_json::Value = serde_json::from_str(&encoded).unwrap();
    assert_eq!(wire["application_id"], "fixture-app");
    assert_eq!(wire["version"], PROJECT_FILE_VERSION);
    assert!(wire.get("root").is_some());
    assert!(wire.get("project").is_none());
}

#[test]
fn application_codec_rejects_a_foreign_identity_on_encode_and_decode() {
    let document = fixture();
    let encoded = encode_application_project_document(&document, "fixture-app").unwrap();
    for error in [
        encode_application_project_document(&document, "other-app").unwrap_err(),
        decode_application_project_document::<serde_json::Value>(&encoded, "other-app").unwrap_err(),
    ] {
        assert!(
            matches!(error, ProjectDocumentCodecError::ApplicationMismatch { found, expected }
            if found == "fixture-app" && expected == "other-app")
        );
    }
}

#[test]
fn a_generic_graph_document_is_not_an_application_project() {
    let encoded = encode_project_document(&fixture().project).unwrap();
    assert!(matches!(
        decode_application_project_document::<serde_json::Value>(&encoded, "fixture-app"),
        Err(ProjectDocumentCodecError::MissingApplicationId)
    ));
}

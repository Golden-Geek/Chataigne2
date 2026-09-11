use golden_model::{NodeUuid, UserNodeRole};

use crate::{
    PROJECT_FILE_VERSION, ProjectDocument, ProjectDocumentCodecError, ProjectNodeRecord, decode_project_document,
    encode_project_document,
};

fn fixture() -> ProjectDocument {
    ProjectDocument {
        version: PROJECT_FILE_VERSION.to_string(),
        ui_state: Some(serde_json::json!({ "panel": "graph" })),
        root: ProjectNodeRecord {
            uuid: NodeUuid::nil(),
            node_type: "root".to_string(),
            user_role: UserNodeRole::Regular,
            meta: serde_json::json!({ "label": "Project" }),
            data: None,
            children: vec![ProjectNodeRecord {
                uuid: NodeUuid::nil(),
                node_type: "folder".to_string(),
                user_role: UserNodeRole::ItemRoot,
                meta: serde_json::Value::Null,
                data: Some(serde_json::json!({ "expanded": true })),
                children: Vec::new(),
            }],
        },
    }
}

#[test]
fn typed_project_codec_round_trips_without_an_engine_or_host() {
    let document = fixture();
    let encoded = encode_project_document(&document).expect("fixture should encode");
    let decoded = decode_project_document(&encoded).expect("fixture should decode");

    assert_eq!(decoded, document);
    assert!(!encoded.contains("\"meta\": null"), "empty metadata should be omitted");
}

#[test]
fn codec_rejects_an_unsupported_project_version() {
    let mut document = fixture();
    document.version = "999".to_string();

    assert!(matches!(
        encode_project_document(&document),
        Err(ProjectDocumentCodecError::UnsupportedVersion { found, expected })
            if found == "999" && expected == PROJECT_FILE_VERSION
    ));
}

use golden_model::{NodeUuid, UserNodeRole};
use golden_persistence::{
    PROJECT_FILE_VERSION, ProjectDocument, ProjectNodeRecord, decode_project_document, encode_project_document,
};

#[test]
fn public_codec_round_trips_without_engine_or_host_types() {
    let document = ProjectDocument {
        version: PROJECT_FILE_VERSION.to_string(),
        ui_state: Some(serde_json::json!({"panel": "graph"})),
        root: ProjectNodeRecord {
            uuid: NodeUuid::nil(),
            node_type: "fixture.root".to_string(),
            user_role: UserNodeRole::Regular,
            meta: serde_json::json!({"owner": "external-consumer"}),
            data: Some(serde_json::json!({"value": 42})),
            children: Vec::new(),
        },
    };

    let encoded = encode_project_document(&document).expect("public persistence codec should encode");
    let decoded: ProjectDocument = decode_project_document(&encoded).expect("public persistence codec should decode");

    assert_eq!(decoded, document);
}

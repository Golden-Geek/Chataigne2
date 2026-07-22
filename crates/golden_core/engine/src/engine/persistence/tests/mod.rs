use uuid::Uuid;

use super::{PROJECT_FILE_VERSION, ProjectFile, ProjectNodeMeta, ProjectNodeRecord};
use crate::define_node_enum;
use crate::edit::Edit;
use crate::engine::Engine;
use crate::node::{FOLDER_NODE_TYPE, Folder, Node, NodeId, NodeUuid, UserNodeRole};
use crate::parameter::{ParamValue, ParameterEventBehaviour};
use crate::process_ctx::ProcessCtx;

#[crate::node("recover_bad_init_node")]
struct RecoverBadInitNode {}

#[crate::node("recover_bad_init_node", from_struct)]
impl Node for RecoverBadInitNode {
    fn init(&mut self, ctx: &mut ProcessCtx) {
        ctx.edits.push(Edit::SetParam {
            node: NodeId(u64::MAX),
            value: ParamValue::Int(1),
            behaviour: ParameterEventBehaviour::Coalesce,
        });
    }
}

define_node_enum!(
    enum RecoveryTestNode {
        RecoverBadInitNode,
    }
);

fn project_record(node_type: &str, label: &str, children: Vec<ProjectNodeRecord>) -> ProjectNodeRecord {
    ProjectNodeRecord {
        uuid: NodeUuid(Uuid::new_v4()),
        node_type: node_type.to_string(),
        user_role: UserNodeRole::Regular,
        meta: ProjectNodeMeta {
            label: Some(label.to_string()),
            ..ProjectNodeMeta::default()
        },
        data: None,
        children,
    }
}

fn recovery_test_project() -> ProjectFile {
    ProjectFile {
        version: PROJECT_FILE_VERSION.to_string(),
        ui_state: None,
        root: project_record(
            FOLDER_NODE_TYPE,
            "root",
            vec![project_record("recover_bad_init_node", "bad init", Vec::new())],
        ),
    }
}

#[test]
fn project_file_recovery_reports_and_skips_rebuild_error() {
    let project = recovery_test_project();
    let strict_error =
        match Engine::<RecoveryTestNode>::from_project_file_with(project.clone(), |node_type, _data, _meta| {
            match node_type {
                FOLDER_NODE_TYPE => Ok(Folder::new("root").into()),
                "recover_bad_init_node" => Ok(RecoverBadInitNode::new().into()),
                _ => Err(format!("unknown node type '{node_type}'")),
            }
        }) {
            Ok(_) => panic!("strict load should reject the load-time invalid edit"),
            Err(error) => error,
        };
    assert!(
        strict_error.to_string().contains("SetParam"),
        "strict error should describe the failed edit: {strict_error}"
    );

    let (loaded_engine, recovery) = Engine::<RecoveryTestNode>::from_project_file_with_recovery(
        project,
        |node_type, _data, _meta| match node_type {
            FOLDER_NODE_TYPE => Ok(Folder::new("root").into()),
            "recover_bad_init_node" => Ok(RecoverBadInitNode::new().into()),
            _ => Err(format!("unknown node type '{node_type}'")),
        },
    )
    .expect("recovering load should keep the valid graph");

    assert_eq!(loaded_engine.nodes.iter().count(), 2);
    assert_eq!(recovery.problems.len(), 1);
    assert!(
        recovery.problems[0].message.contains("SetParam"),
        "recovery problem should describe the skipped edit: {:?}",
        recovery.problems[0]
    );
}

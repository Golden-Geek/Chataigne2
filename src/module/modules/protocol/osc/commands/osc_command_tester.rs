use golden_core::{
    node,
    node::{Node, NodeId, UserCreatableItem},
    process_ctx::ProcessCtx,
};

#[node("osc_command_tester", label = "Command Tester")]
pub struct OscCommandTester {
    manager: crate::app::ModuleCommandManagerBase,
}

impl OscCommandTester {
    pub fn create() -> Self {
        Self::new(crate::app::ModuleCommandManagerBase::new())
    }
}

#[node("osc_command_tester", via = manager, from_struct)]
impl Node for OscCommandTester {
    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == "osc_command_tester").then(Self::create)
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        vec![
            UserCreatableItem::new(
                crate::app::OSC_SEND_CUSTOM_MESSAGE_COMMAND_NODE_TYPE,
                crate::app::module_command::MODULE_COMMAND_ITEM_KIND,
                "Send Custom Message",
            )
            .with_select_when_created(false),
        ]
    }

    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        match node_type {
            crate::app::OSC_SEND_CUSTOM_MESSAGE_COMMAND_NODE_TYPE => {
                Some(Box::new(crate::app::OscSendCustomMessageCommand::create()))
            }
            _ => None,
        }
    }

    fn on_child_added(&mut self, ctx: &mut ProcessCtx, parent: NodeId, child: NodeId) {
        if parent == self.id() {
            self.manager.ensure_command_tester_controls(ctx, child);
        }
    }
}

#[cfg(test)]
mod tests {
    use golden_core::app::ProjectNode;
    use golden_core::node::Node;
    use golden_core::node::NodeMeta;

    use super::OscCommandTester;

    #[test]
    fn new_commands_do_not_auto_select() {
        let tester = OscCommandTester::create();
        let command_item = tester
            .user_creatable_items()
            .into_iter()
            .find(|item| item.node_type == crate::app::OSC_SEND_CUSTOM_MESSAGE_COMMAND_NODE_TYPE)
            .expect("send custom message command item should be listed");

        assert!(
            !command_item.select_when_created,
            "command tester items should not auto-select newly created commands"
        );
    }

    #[test]
    fn command_tester_does_not_create_folders() {
        let tester = OscCommandTester::create();

        assert!(
            tester.create_user_item("folder").is_none(),
            "command tester should not create command folders"
        );
        assert!(
            tester
                .user_creatable_items()
                .into_iter()
                .all(|item| item.node_type != "folder" && item.item_kind != "folder"),
            "command tester catalog should not advertise command folders"
        );
    }

    #[test]
    fn osc_command_tester_decodes_from_project_node_type() {
        let node = <crate::app::AppNode as ProjectNode>::project_decode_node(
            "osc_command_tester",
            &serde_json::Value::Null,
            &NodeMeta::new("Command Tester".to_string()),
        )
        .expect("osc command tester should decode from project files");

        assert_eq!(node.get_type(), "osc_command_tester");
    }
}

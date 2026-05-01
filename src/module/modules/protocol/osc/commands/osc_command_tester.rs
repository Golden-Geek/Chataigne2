use golden_core::{
    node,
    node::{Node, UserCreatableItem},
};

#[node("osc_command_tester", label = "Command Tester")]
pub struct OscCommandTester {
    base: crate::app::ModuleCommandsContainer,
}

impl OscCommandTester {
    pub fn create() -> Self {
        Self::new(crate::app::ModuleCommandsContainer::new())
    }
}

#[node("osc_command_tester", via = base, from_struct)]
impl Node for OscCommandTester {
    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        vec![
            UserCreatableItem::new(
                crate::app::OSC_SEND_CUSTOM_MESSAGE_COMMAND_NODE_TYPE,
                crate::app::module_command::MODULE_COMMAND_ITEM_KIND,
                "Send Custom Message",
            )
            .with_select_when_created(false),
            UserCreatableItem::new("folder", "folder", "Folder").with_select_when_created(false),
        ]
    }

    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        match node_type {
            crate::app::OSC_SEND_CUSTOM_MESSAGE_COMMAND_NODE_TYPE => {
                Some(Box::new(crate::app::OscSendCustomMessageCommand::create()))
            }
            "folder" => Some(Box::new(crate::app::module_command::create_command_folder())),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use golden_core::node::Node;

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
    fn created_command_folders_stay_visible_in_nested_inspectors() {
        let tester = OscCommandTester::create();
        let folder = tester
            .create_user_item("folder")
            .expect("folder creation should be supported");

        assert!(
            folder.node_data().meta.presentation.show_in_nested_inspector,
            "command tester folders should stay visible without requiring selection"
        );
    }
}

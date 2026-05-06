use golden_core::node::Node;

use super::MqttPublishCommand;

#[test]
fn mqtt_publish_command_is_a_module_command_item() {
    assert_eq!(
        MqttPublishCommand::create().user_item_kind(),
        crate::app::module_command::MODULE_COMMAND_ITEM_KIND
    );
}

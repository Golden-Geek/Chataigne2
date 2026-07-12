use golden_core::node::{DeclaredUserItemNode, Node};

use super::ButtplugSetOutputCommand;

#[test]
fn buttplug_commands_are_module_command_items() {
    let command_types = [
        crate::app::ButtplugSetOutputCommand::NODE_TYPE,
        crate::app::ButtplugStopDeviceCommand::NODE_TYPE,
        crate::app::ButtplugStopAllDevicesCommand::NODE_TYPE,
        crate::app::ButtplugStartScanningCommand::NODE_TYPE,
        crate::app::ButtplugStopScanningCommand::NODE_TYPE,
    ];

    for command_type in command_types {
        assert!(crate::app::declared_user_item_type_matches(
            command_type,
            crate::app::module_command::MODULE_COMMAND_ITEM_KIND
        ));
    }
}

#[test]
fn set_output_command_defaults_to_selected_vibrate_zero() {
    let command = ButtplugSetOutputCommand::create();
    assert_eq!(
        <ButtplugSetOutputCommand as DeclaredUserItemNode>::ITEM_KIND,
        crate::app::module_command::MODULE_COMMAND_ITEM_KIND
    );
    assert_eq!(command.get_type(), crate::app::ButtplugSetOutputCommand::NODE_TYPE);
}

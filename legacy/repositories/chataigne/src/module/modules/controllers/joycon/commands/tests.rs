use golden_core::node::{DeclaredUserItemNode, Node};

use super::{JoyConSetLedCommand, JoyConVibrateCommand};

#[test]
fn joycon_commands_are_module_command_items() {
    for command_type in [
        crate::app::JoyConVibrateCommand::NODE_TYPE,
        crate::app::JoyConSetLedCommand::NODE_TYPE,
    ] {
        assert!(crate::app::declared_user_item_type_matches(
            command_type,
            crate::app::module_command::MODULE_COMMAND_ITEM_KIND
        ));
    }
}

#[test]
fn joycon_command_constructors_match_declared_node_types() {
    let vibrate = JoyConVibrateCommand::create();
    let set_led = JoyConSetLedCommand::create();

    assert_eq!(
        <JoyConVibrateCommand as DeclaredUserItemNode>::ITEM_KIND,
        crate::app::module_command::MODULE_COMMAND_ITEM_KIND
    );
    assert_eq!(vibrate.get_type(), crate::app::JoyConVibrateCommand::NODE_TYPE);
    assert_eq!(set_led.get_type(), crate::app::JoyConSetLedCommand::NODE_TYPE);
}

#[test]
fn joycon_commands_default_to_fixed_controller_slots() {
    assert_eq!(crate::app::module::common::joycon::JoyConControllerTarget::Both.variant_id(), "both");
}

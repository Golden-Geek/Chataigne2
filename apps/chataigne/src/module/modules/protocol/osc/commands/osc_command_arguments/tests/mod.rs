use golden_core::node::Node;

use super::OscCommandArguments;

#[test]
fn new_argument_parameters_do_not_auto_select() {
    let arguments = OscCommandArguments::create();

    assert!(
        arguments
            .user_creatable_items()
            .into_iter()
            .all(|item| !item.select_when_created),
        "manual OSC argument parameters should not auto-select when created"
    );
}

#[test]
fn created_argument_parameters_stay_visible_in_nested_inspectors() {
    let arguments = OscCommandArguments::create();
    let parameter = arguments
        .create_user_item("float")
        .expect("float argument creation should be supported");

    assert!(
        parameter.node_data().meta.presentation.show_in_nested_inspector,
        "manual OSC argument parameters should remain visible without selection"
    );
}

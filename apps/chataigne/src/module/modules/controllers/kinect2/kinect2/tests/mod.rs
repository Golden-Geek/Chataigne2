use super::Kinect2Module;
use golden_core::{node::DeclaredUserItemNode, script::ScriptSource};
pub(super) mod fixture;
#[test]
fn kinect2_module_is_a_module_item() {
    assert_eq!(
        <Kinect2Module as DeclaredUserItemNode>::ITEM_KIND,
        crate::app::module::MODULE_ITEM_KIND
    );
    assert!(crate::app::declared_user_item_type_matches(
        Kinect2Module::NODE_TYPE,
        crate::app::module::MODULE_ITEM_KIND
    ));
}

#[test]
fn reference_space_changes_joint_output_origin() {
    super::service::tests::reference_space_changes_joint_output_origin();
}
#[test]
fn hand_metrics_compute_distance_rotation_and_speed() {
    super::service::tests::hand_metrics_compute_distance_rotation_and_speed();
}
#[test]
fn kinect2_script_template_resolves_to_inline_module_template() {
    let config = crate::app::module::script_api::module_script_config(Kinect2Module::NODE_TYPE);
    let ScriptSource::Inline { text: source } = config.source else {
        panic!("kinect2 module script template should resolve to inline source");
    };

    assert!(source.contains("function moduleConnectionChanged"));
    assert!(!source.contains("function noteOnReceived"));
}

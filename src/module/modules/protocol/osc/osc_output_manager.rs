use golden_core::{node, node::Node};

#[node("osc_output_manager", label = "Outputs")]
#[children(
    node default_output: crate::app::OscOutput = crate::app::OscOutput::create_with_module_authoring() (
        label = "Output"
    );
)]
pub struct OscOutputManager {}

impl OscOutputManager {
    pub fn create_with_module_authoring() -> Self {
        let mut manager = Self::new();
        crate::app::module::enable_module_authoring(manager.node_data_mut());
        manager
    }
}

#[node("osc_output_manager", from_struct)]
impl Node for OscOutputManager {
    golden_core::define_user_item_factory_methods! {
        accepts = ["osc_output", "folder"];
        items = [
            {
                node_type: "osc_output",
                item_kind: "osc_output",
                label: "Output",
                select_when_created: false,
                create: |_this: &Self| crate::app::OscOutput::create_with_module_authoring()
            }
        ];
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::create_with_module_authoring)
    }
}

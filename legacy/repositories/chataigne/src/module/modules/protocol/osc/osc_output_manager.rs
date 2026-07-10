use golden_core::{node, node::Node, process_ctx::ProcessCtx};

#[node("osc_output_manager", label = "Outputs")]
pub struct OscOutputManager {}

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
                create: |_this: &Self| crate::app::OscOutput::new()
            }
        ];
    }

    fn init(&mut self, _ctx: &mut ProcessCtx) {
        crate::app::module::enable_module_authoring(self.node_data_mut());
    }
}

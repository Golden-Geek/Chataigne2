use golden_core::{node, node::Node, process_ctx::ProcessCtx};

#[node("module_manager", label = "Module Manager")]
pub struct ModuleManager {}

#[node("module_manager", from_struct)]
impl Node for ModuleManager {
    golden_core::define_user_item_factory_methods! {
        accepts = ["module", "folder"];
        items = [
            {
                node_type: "module_base",
                item_kind: "module",
                label: "Module",
                create: |_this: &Self| crate::app::ModuleBase::new()
            },
            {
                node_type: "folder",
                item_kind: "folder",
                label: "Folder",
                create: |_this: &Self| golden_core::node::Folder::new("Folder")
            },
        ];
    }

    fn init(&mut self, _ctx: &mut ProcessCtx) {
        crate::app::module::enable_module_authoring(self.node_data_mut());
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AppNodeDescriptor {
    pub type_id: &'static str,
    pub display_name: &'static str,
    pub user_item_kind: Option<&'static str>,
}

pub(crate) const MODULE_ITEM_KIND: &str = "module";
pub(crate) const SCRIPT_ITEM_KIND: &str = "script";
pub(crate) const SCRIPT_NODE_TYPE: &str = "script";

pub(crate) const MODULE_MANAGER: AppNodeDescriptor = AppNodeDescriptor {
    type_id: "module_manager",
    display_name: "Module Manager",
    user_item_kind: None,
};

pub(crate) const MODULE_BASE: AppNodeDescriptor = AppNodeDescriptor {
    type_id: "module_base",
    display_name: "Module",
    user_item_kind: Some(MODULE_ITEM_KIND),
};

pub(crate) const OSC_MODULE_BASE: AppNodeDescriptor = AppNodeDescriptor {
    type_id: "osc_module_base",
    display_name: "OSC Module",
    user_item_kind: Some(MODULE_ITEM_KIND),
};

pub(crate) const GENERIC_OSC_MODULE: AppNodeDescriptor = AppNodeDescriptor {
    type_id: "generic_osc_module",
    display_name: "Generic OSC Module",
    user_item_kind: Some(MODULE_ITEM_KIND),
};

pub(crate) const FOLDER_ITEM: AppNodeDescriptor = AppNodeDescriptor {
    type_id: golden_core::node::FOLDER_NODE_TYPE,
    display_name: "Folder",
    user_item_kind: Some(golden_core::node::FOLDER_NODE_TYPE),
};

pub(crate) const SCRIPT_ITEM: AppNodeDescriptor = AppNodeDescriptor {
    type_id: SCRIPT_NODE_TYPE,
    display_name: "Script",
    user_item_kind: Some(SCRIPT_ITEM_KIND),
};

pub(crate) const MODULE_INFOS_DECL_ID: &str = "infos";
pub(crate) const MODULE_PARAMETERS_DECL_ID: &str = "parameters";
pub(crate) const MODULE_VALUES_DECL_ID: &str = "values";
pub(crate) const MODULE_VALUES_REFERENCE_FILTER_KEY: &str = "module_values_parameters";

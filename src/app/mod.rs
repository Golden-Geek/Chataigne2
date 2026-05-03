mod bootstrap;
mod default_project;

include!(concat!(env!("OUT_DIR"), "/app_nodes.rs"));
include!(concat!(env!("OUT_DIR"), "/ui_assets.rs"));

pub type AppEngine = golden_core::engine::Engine<AppNode>;
pub(crate) use module_modules_protocol_osc_commands_osc_send_custom_message_command::OSC_SEND_CUSTOM_MESSAGE_COMMAND_NODE_TYPE;
pub(crate) use module_modules_protocol_osc_osc_module_base::{
    OscDecodedMessage, OscIncomingApplyResult, OscSendCustomMessageRequest, OscValuePayload,
};

const APP_UI_DEV_SERVER: golden_core::app::FrontendDevServerConfig = golden_core::app::FrontendDevServerConfig {
    working_dir: concat!(env!("CARGO_MANIFEST_DIR"), "/src-ui"),
    npm_script: "dev",
    url: "http://127.0.0.1:5173",
};

pub fn run() -> std::io::Result<()> {
    golden_core::app::run_default_with_ui_assets_and_dev_server::<AppNode, tauri::Wry>(
        tauri::generate_context!(),
        APP_UI_ASSETS,
        Some(APP_UI_DEV_SERVER),
    )
}

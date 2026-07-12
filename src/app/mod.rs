mod bootstrap;
mod default_project;

include!(concat!(env!("OUT_DIR"), "/app_nodes.rs"));
include!(concat!(env!("OUT_DIR"), "/ui_assets.rs"));

pub type AppEngine = golden_core::engine::Engine<AppNode>;
pub(crate) use module_modules_protocol_midi_commands::MidiSendRequest;
pub(crate) use module_modules_protocol_midi_midi_module::midi_message::{
    cc_supports_14_bit, clamp_channel_i32, clamp_i32_to_u14, clamp_i32_to_u7, encode_14_bit_control_change,
    encode_midi_message, message_description, normalize_sysex_bytes, note_pitch_from_name_octave, MidiMessage,
    MidiSystemMessage,
};
pub(crate) use module_modules_protocol_osc_commands_osc_send_custom_message_command::OSC_SEND_CUSTOM_MESSAGE_COMMAND_NODE_TYPE;
pub(crate) use module_modules_protocol_osc_osc_module_base::{
    OscDecodedMessage, OscSendCustomMessageRequest, OscValuePayload,
};

const APP_UI_DEV_SERVER: golden_core::app::FrontendDevServerConfig = golden_core::app::FrontendDevServerConfig {
    working_dir: concat!(env!("CARGO_MANIFEST_DIR"), "/src-ui"),
    npm_script: "dev",
    url: "http://127.0.0.1:5173",
};

pub fn run() -> std::io::Result<()> {
    if crate::product_evidence::try_run_from_env()? {
        return Ok(());
    }

    golden_core::app::run_default_with_ui_assets_and_dev_server::<AppNode, tauri::Wry>(
        tauri::generate_context!(),
        APP_UI_ASSETS,
        Some(APP_UI_DEV_SERVER),
    )
}

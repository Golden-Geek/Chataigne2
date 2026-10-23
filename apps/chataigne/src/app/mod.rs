mod bootstrap;
mod default_project;

include!(concat!(env!("OUT_DIR"), "/app_nodes.rs"));
include!(concat!(env!("OUT_DIR"), "/ui_assets.rs"));

pub type AppEngine = golden_core::engine::Engine<AppNode>;
pub(crate) use module_modules_controllers_buttplug::queue_buttplug_command;
pub(crate) use module_modules_controllers_joycon::queue_joycon_command;
pub(crate) use module_modules_controllers_keyboard_keyboard::queue_keyboard_command;
pub(crate) use module_modules_controllers_mouse_mouse::queue_mouse_command;
pub(crate) use module_modules_protocol_dmx::queue_dmx_command;
pub(crate) use module_modules_protocol_midi_commands::{
    append_midi_bytes, normalize_midi_sysex, prepare_midi_command, MidiSendPacket, MidiSendRequest,
};
pub(crate) use module_modules_protocol_midi_midi_module::midi_message::{
    cc_supports_14_bit, clamp_channel_i32, clamp_i32_to_u14, clamp_i32_to_u7, encode_14_bit_control_change,
    encode_midi_message, message_description, note_pitch_from_name_octave, MidiMessage, MidiSystemMessage,
};
pub(crate) use module_modules_protocol_mqtt::queue_mqtt_command;
pub(crate) use module_modules_protocol_osc_commands_osc_send_custom_message_command::OSC_SEND_CUSTOM_MESSAGE_COMMAND_NODE_TYPE;
pub(crate) use module_modules_protocol_osc_osc_module_base::{queue_osc_command, OscOutboundRequest, OscQueueError};
pub(crate) use module_modules_system_app_control_app_control::queue_app_control_command;
pub(crate) use module_modules_system_os_os::queue_os_command;

const APP_UI_DEV_SERVER: golden_core::app::FrontendDevServerConfig = golden_core::app::FrontendDevServerConfig {
    working_dir: concat!(env!("CARGO_MANIFEST_DIR"), "/ui"),
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

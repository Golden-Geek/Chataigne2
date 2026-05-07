use golden_core::{
    node::DeclaredUserItemNode,
    script::ScriptSource,
};

const COMMON_FUNCTION_DOCS: &[&str] = &[
    "Module host quick reference:",
    "local.getChild(indexOrKey)",
    "local.addParameter(name, defaultValueOrSpec)",
    "local.listen({ level: 2 })",
];

const MIDI_FUNCTION_DOCS: &[&str] = &[
    "MIDI module functions",
    "local.sendNoteOn(channel, note, velocity = 127)",
    "local.sendControlChange(channel, controller, value)",
    "local.sendRawBytes(...bytes)",
];

const OSC_FUNCTION_DOCS: &[&str] = &[
    "OSC module functions",
    "local.sendMessage(address, ...values)",
    "local.sendOSC(address, ...values)",
];

const STREAM_FUNCTION_DOCS: &[&str] = &[
    "Stream module functions",
    "local.sendText(text, lineEnding = \"none\")",
    "local.sendHexString(hex)",
];

const GAMEPAD_CALLBACK_DOCS: &[&str] = &[
    "Gamepad module callbacks",
    "gamepadAxisChanged(axis, value, rawValue, gamepad)",
    "gamepadButtonPressed(button, value, gamepad)",
];

const SERVER_STREAM_FUNCTION_DOCS: &[&str] = &[
    "Server stream modules use the stream send functions above to broadcast",
    "source client id",
];

#[test]
fn module_script_templates_document_available_functions_for_each_module() {
    let osc_module_type = <crate::app::GenericOscModule as DeclaredUserItemNode>::ITEM_NODE_TYPE;
    let cases: &[(&str, &[&str], &[&str])] = &[
        ("module_base", &[], &[]),
        (crate::app::GamepadModule::NODE_TYPE, GAMEPAD_CALLBACK_DOCS, &[]),
        (crate::app::MidiModule::NODE_TYPE, MIDI_FUNCTION_DOCS, &[]),
        (osc_module_type, OSC_FUNCTION_DOCS, &[]),
        (crate::app::SerialModule::NODE_TYPE, STREAM_FUNCTION_DOCS, &[]),
        (crate::app::UdpModule::NODE_TYPE, STREAM_FUNCTION_DOCS, &[]),
        (crate::app::TcpClientModule::NODE_TYPE, STREAM_FUNCTION_DOCS, &[]),
        (
            crate::app::TcpServerModule::NODE_TYPE,
            STREAM_FUNCTION_DOCS,
            SERVER_STREAM_FUNCTION_DOCS,
        ),
        (crate::app::WebSocketClientModule::NODE_TYPE, STREAM_FUNCTION_DOCS, &[]),
        (
            crate::app::WebSocketServerModule::NODE_TYPE,
            STREAM_FUNCTION_DOCS,
            SERVER_STREAM_FUNCTION_DOCS,
        ),
    ];

    for (host_node_type, family_docs, server_docs) in cases {
        let source = inline_source_for(host_node_type);
        assert_contains_all(host_node_type, source.as_str(), COMMON_FUNCTION_DOCS);
        assert_contains_all(host_node_type, source.as_str(), family_docs);
        assert_contains_all(host_node_type, source.as_str(), server_docs);
    }
}

fn inline_source_for(host_node_type: &str) -> String {
    let config = super::module_script_config(host_node_type);
    let ScriptSource::Inline(source) = config.source else {
        panic!("module script config should resolve to inline source for {host_node_type}");
    };
    source
}

fn assert_contains_all(host_node_type: &str, source: &str, snippets: &[&str]) {
    for snippet in snippets {
        assert!(
            source.contains(snippet),
            "module template for {host_node_type} should document '{snippet}'; source was:\n{source}"
        );
    }
}

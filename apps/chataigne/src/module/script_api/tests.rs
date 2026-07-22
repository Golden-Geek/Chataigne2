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

const JOYCON_FUNCTION_DOCS: &[&str] = &[
    "Joy-Con module functions",
    "local.vibrate(frequencyHz = 300, amplitude = 0.9, durationMs = 60, target = \"both\")",
    "local.setPlayerLights(led1 = \"off\", led2 = \"off\", led3 = \"off\", led4 = \"off\", target = \"both\")",
];

const JOYCON_CALLBACK_DOCS: &[&str] = &[
    "Joy-Con module callbacks",
    "joyConConnected(side, joyCon)",
    "joyConButtonPressed(side, button, joyCon)",
    "joyConStickChanged(side, stick, joyCon)",
    "joyConMotionChanged(side, motion, joyCon)",
];

const BUTTPLUG_FUNCTION_DOCS: &[&str] = &[
    "Buttplug module functions",
    "local.vibrate(value, device = \"selected\")",
    "local.setOutput(output, value, device = \"selected\", durationMs = 1000)",
    "local.positionWithDuration(value, durationMs = 1000, device = \"selected\")",
];

const MOUSE_FUNCTION_DOCS: &[&str] = &[
    "Mouse module functions",
    "local.moveMouse(x, y, coordinate = \"absolute\", units = \"pixels\")",
    "local.click(button = \"left\")",
    "local.scroll(vertical, horizontal = 0)",
];

const BUTTPLUG_CALLBACK_DOCS: &[&str] = &[
    "buttplugDeviceAdded(device)",
    "buttplugScanningFinished()",
];

const MOUSE_CALLBACK_DOCS: &[&str] = &[
    "Mouse module callbacks",
    "mouseMoved(position, delta, mouse)",
    "mouseButtonPressed(button, mouse)",
    "mouseButtonReleased(button, mouse)",
];

const SERVER_STREAM_FUNCTION_DOCS: &[&str] = &[
    "Server stream modules use the stream send functions above to broadcast",
    "source client id",
];

const DMX_FUNCTION_DOCS: &[&str] = &[
    "Art-Net and sACN DMX functions",
    "local.setChannel(channel, value)",
    "local.sendFrame(\"[0, 127, 255]\")",
    "local.blackout()",
];

const DMX_CALLBACK_DOCS: &[&str] = &["dmxFrameReceived(universe, channels, metadata)"];

const NODE_FUNCTION_DOCS: &[&str] = &[
    "Node module functions",
    "local.setValue(targetReference, value)",
    "local.trigger(targetReference)",
];

const NODE_CALLBACK_DOCS: &[&str] = &[
    "nodeValueSet(target, value)",
    "nodeTriggered(target)",
];

const SIGNALS_FUNCTION_DOCS: &[&str] = &[
    "Signals module functions",
    "local.resetSignals()",
    "local.resetSignal(nameOrIndex)",
];

const SIGNALS_CALLBACK_DOCS: &[&str] = &["signalCycle(name, cycles, details)"];

const METRONOMES_FUNCTION_DOCS: &[&str] = &[
    "Metronomes module functions",
    "local.resetMetronomes()",
    "local.resetMetronome(nameOrIndex)",
    "local.tickMetronome(nameOrIndex)",
];

const METRONOMES_CALLBACK_DOCS: &[&str] = &["metronomeTick(name, ticks, totalTicks, details)"];

const SPATIALIZER_FUNCTION_DOCS: &[&str] = &["Spatializer module values", "local.values"];

#[test]
fn module_script_templates_document_available_functions_for_each_module() {
    let osc_module_type = <crate::app::GenericOscModule as DeclaredUserItemNode>::ITEM_NODE_TYPE;
    let cases: &[(&str, &[&str], &[&str])] = &[
        ("module_base", &[], &[]),
        (
            crate::app::ButtplugModule::NODE_TYPE,
            BUTTPLUG_FUNCTION_DOCS,
            BUTTPLUG_CALLBACK_DOCS,
        ),
        (crate::app::GamepadModule::NODE_TYPE, GAMEPAD_CALLBACK_DOCS, &[]),
        (
            crate::app::JoyConModule::NODE_TYPE,
            JOYCON_FUNCTION_DOCS,
            JOYCON_CALLBACK_DOCS,
        ),
        (
            crate::app::MouseModule::NODE_TYPE,
            MOUSE_FUNCTION_DOCS,
            MOUSE_CALLBACK_DOCS,
        ),
        (crate::app::MidiModule::NODE_TYPE, MIDI_FUNCTION_DOCS, &[]),
        (
            crate::app::ArtNetModule::NODE_TYPE,
            DMX_FUNCTION_DOCS,
            DMX_CALLBACK_DOCS,
        ),
        (
            crate::app::SacnModule::NODE_TYPE,
            DMX_FUNCTION_DOCS,
            DMX_CALLBACK_DOCS,
        ),
        (
            crate::app::NodeModule::NODE_TYPE,
            NODE_FUNCTION_DOCS,
            NODE_CALLBACK_DOCS,
        ),
        (
            crate::app::SignalsModule::NODE_TYPE,
            SIGNALS_FUNCTION_DOCS,
            SIGNALS_CALLBACK_DOCS,
        ),
        (
            crate::app::MetronomesModule::NODE_TYPE,
            METRONOMES_FUNCTION_DOCS,
            METRONOMES_CALLBACK_DOCS,
        ),
        (
            crate::app::SpatializerModule::NODE_TYPE,
            SPATIALIZER_FUNCTION_DOCS,
            &[],
        ),
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

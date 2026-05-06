# Module Scripting

Module script callbacks are app-owned Chataigne2 behavior. The generic runtime in `golden_core`
only dispatches named callbacks from custom event payloads; callback names, argument shapes, and
module send methods live with the module family that owns them.

Default module script source comes from `src/module/script_templates/`. Each concrete module gets
its own template file, such as `midi_module.js`, `osc_module.js`, `serial_module.js`, or
`websocket_server_module.js`, and those files compose shared snippets with `{{include:...}}`.
Reusable JS documentation snippets in `src/module/script_templates/snippets/` are comment-only
quick references for the functions available on `local`, the module host proxy.

## Common Callbacks

All scriptable modules expose parameter-change callbacks for their standard module folders:

- `moduleConnectionChanged(param, newValue, oldValue, change)`
- `moduleModuleParameterChanged(param, newValue, oldValue, change)`
- `moduleModuleValueChanged(param, newValue, oldValue, change)`

`param` is the changed parameter node handle. `change` contains the parameter path plus serialized
new and old values.

## Protocol Callbacks

MIDI modules emit:

- `midiMessageReceived(message)`
- `noteOnReceived(channel, note, velocity)`
- `noteOffReceived(channel, note, velocity)`
- `ccReceived(channel, controller, value)`
- `sysExReceived(bytes)`

OSC modules emit:

- `messageReceived(address, payload, message)`

Streaming modules emit:

- `textReceived(text, source)`
- `dataReceived(bytes, source)`
- `dataReceive(bytes, source)`

Server-style stream modules also emit:

- `clientConnected(clientId, info)`
- `clientDisconnected(clientId, reason)`

`source` is `null` for single-peer transports and a client id for server transports.

## Send Methods

MIDI modules expose:

- `sendNoteOn(channel, note, velocity)`
- `sendNoteOff(channel, note, velocity)`
- `sendFullNote(channel, note, velocity, durationMs, offVelocity)`
- `sendCC(channel, controller, value)`
- `sendControlChange(channel, controller, value)`
- `sendProgramChange(channel, program)`
- `sendPitchBend(channel, value)`
- `sendChannelPressure(channel, pressure)`
- `sendPolyPressure(channel, note, pressure)`
- `sendSysEx(...bytes)`
- `sendSysex(...bytes)`
- `sendRawBytes(...bytes)`

OSC modules expose `sendMessage(address, ...values)`, `sendOSC(address, ...values)`, and
`sendOsc(address, ...values)`.

Streaming modules expose:

- `sendText(text, lineEnding)`
- `sendString(text, lineEnding)`
- `sendBytes(...bytes)`
- `sendData(...bytes)`
- `sendHex(hex)`
- `sendHexString(hex)`

Concrete modules should add script methods and callback constants at their own boundary and delegate
shared parsing or payload construction to app-owned family helpers. Do not add Chataigne2-specific
module scripting callbacks to `golden_core` templates or engine modules.

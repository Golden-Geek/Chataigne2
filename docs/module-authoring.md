# Module Authoring

A Chataigne module is one app-owned vertical feature. It is incomplete until its runtime node,
commands, script-callable methods, script callbacks, template/snippets, persistence, diagnostics,
recovery, assets, and UI extension points are wired at the same boundary.

## Placement and registration

Place the implementation under `apps/chataigne/src/module/modules/<family>/`. Declare its stable
node type, label, module item kind, and menu path beside the `Node` implementation. The public
codegen support builds the app registry; do not maintain a second catalog or edit generated output.

Use the shared module base for enablement, connection state, parameters, values, commands, and
scripts. Reused Params DSL folders inherit their metadata; restate a label only for a deliberate
divergence.

## Runtime and IO

- Give every periodic module a stable `chataigne.runtime.*` compiled-kernel key.
- Parse and timestamp incoming data in a worker before publishing typed bounded input.
- Keep socket, device, filesystem polling, and recurring millisecond work off the engine thread.
- Define queue capacity and overflow semantics. Coalesce replaceable values; preserve ordered
  triggers, commands, and effects.
- Provide reconnect with capped backoff and recovery after endpoint or device replacement.
- Stop, unpark, and join owned workers during node teardown.

Prefer a maintained protocol crate for MIDI, OSC, serial, HID, WebSocket, or similar wire formats.

## Commands and scripts

Commands live with the owning family and expose direct configuration and trigger children. Script
methods and callback constants also live with that family. Add the module template under
`apps/chataigne/src/module/script_templates/` and shared documentation snippets under its
`snippets/` directory. See [module-scripting.md](module-scripting.md) for the published surface.

## Qualification

Cover catalog creation, runtime semantics, command effects, callback payloads, script descriptor
and expansion, sparse save/reload, diagnostics, bounded overload, reconnect, and deterministic
adapter/device recovery. Add an app-owned icon and rendered UI evidence for custom editors. Then
run the focused module suite, `python tools/migration/product_manifest.py check`, and the complete
product gate.

The generic node declaration and controller paging rules are in [adding-a-node.md](adding-a-node.md).

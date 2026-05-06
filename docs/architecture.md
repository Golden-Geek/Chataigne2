# Architecture

The top-level architecture is documented in [../ARCHITECTURE.md](../ARCHITECTURE.md). This page is
the contributor-facing companion for day-to-day placement decisions.

## Ownership

- Chataigne2 app code owns app startup wiring, default project content,
  app-specific node trees, product assets, and Tauri capability configuration.
- `golden_core` owns reusable Rust engine/runtime behavior, protocol DTOs, persistence contracts,
  transport hosting, desktop/headless hosting, native dialogs, macros, and build-time codegen
  support.
- `golden_ui` owns reusable Svelte components, stores, transport clients, host bridges, and
  generated Rust protocol bindings.

## Boundaries

App code should launch through the reusable `golden_core` runtime path and should not reimplement
desktop, headless, transport, protocol, or persistence infrastructure locally. UI code should keep
raw generated wire types separate from UI-local model types.

Stable node identifiers and default item labels live on the node declarations themselves. Concrete
modules declare their `#[node(...)]` type/label and `#[item("module", ...)]` catalog metadata in
the module file; `golden_codegen_support` turns those declarations into the generated app registry
and module creation catalog.

Module command infrastructure lives in `src/module/command`. The reusable command manager base owns
common command-container behavior and accepts command items only; module families such as OSC compose
it to add their concrete command catalog and execution payloads. Command testers start empty, so
modules do not seed ad-hoc commands unless a user or an explicit project preset adds them. Command
items expose trigger and configuration controls as direct children; they should not wrap their
parameters in an extra command folder or persist transient execution results as command parameters.
Command-tester contexts may add tester-only controls such as auto-trigger to command instances
without introducing a separate command type.
When a custom inspector moves a child control into a header or another region, the child should opt
out of the normal inspector content through node presentation metadata instead of being filtered by
node type in the UI. Shared command inspectors should attach to the `module_command` item kind so
all command implementations inherit the same trigger and tester-control placement.

Module scripting is split at the app/runtime boundary. `golden_core` owns generic script runtime
dispatch and node proxy descriptors; Chataigne2 owns module callback names, callback argument
payloads, JavaScript templates, and module-specific send methods in `src/module/`. Module-specific
script API details stay with the owning module family rather than in one global registry. See
[module-scripting.md](module-scripting.md) for the supported module callback and send-method surface.

Inspector visibility is opt-out. `golden_core::node::PresentationHint` defaults nested inspector
visibility on for every node; only nodes that should disappear from a parent inspector should set
`show_in_nested_inspector = false`.
Declared folders and child nodes can set `collapsed = true` in `#[children(...)]`; this is only the
initial UI state, and local user expansion/collapse choices take precedence after interaction.

Generated code is not edited by hand. See [contributor-map.md](contributor-map.md) for the generated
file list and validation checklist.

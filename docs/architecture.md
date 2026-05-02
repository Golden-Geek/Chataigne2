# Architecture

The top-level architecture is documented in [../ARCHITECTURE.md](../ARCHITECTURE.md). This page is
the contributor-facing companion for day-to-day placement decisions.

## Ownership

- Chataigne2 app code owns app startup wiring, app descriptors, default project content,
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

Stable Chataigne app identifiers live in `src/app/descriptors.rs`. Macro attributes still carry the
persisted node type literals required by the node generation system; shared use sites should refer
to descriptors instead of repeating those strings.

Module command infrastructure lives in `src/module/command`. The reusable command manager base owns
common command-container behavior and accepts command items only; module families such as OSC compose
it to add their concrete command catalog and execution payloads. Command testers start empty, so
modules do not seed ad-hoc commands unless a user or an explicit project preset adds them. Command
items expose trigger and configuration controls as direct children; they should not wrap their
parameters in an extra command folder or persist transient execution results as command parameters.
When a custom inspector moves a child control into a header or another region, the child should opt
out of the normal inspector content through node presentation metadata instead of being filtered by
node type in the UI. Shared command inspectors should attach to the `module_command` item kind so
all command implementations inherit the same trigger placement.

Inspector visibility is opt-out. `golden_core::node::PresentationHint` defaults nested inspector
visibility on for every node; only nodes that should disappear from a parent inspector should set
`show_in_nested_inspector = false`.

Generated code is not edited by hand. See [contributor-map.md](contributor-map.md) for the generated
file list and validation checklist.

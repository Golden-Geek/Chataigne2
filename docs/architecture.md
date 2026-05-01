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

Generated code is not edited by hand. See [contributor-map.md](contributor-map.md) for the generated
file list and validation checklist.

# golden_core

`golden_core` is the shared runtime workspace for the Golden engine and related tooling.

The current refactor direction is to keep pure engine and protocol logic separate from desktop host and transport concerns. Desktop and built-in server glue now live in the Chataigne2 app shell instead of the core crate.

## Workspace Crates

- `crates/core`: engine, node model, runtime scheduling, UI DTOs, and app lifecycle helpers still being split into cleaner boundaries.
- `crates/core_macros`: proc macros for node and item declarations.
- `crates/codegen_support`: public build-time helpers intended for app/workspace consumers.

## Design Docs

- `crates/core/docs/dashboard_system.md`
- `crates/core/docs/node_blueprints.md`
- `crates/core/docs/node_contexts.md`
- `crates/core/docs/parameters_control_modes.md`
- `crates/core/docs/scripting_schema.md`

Read these docs before large architectural changes in the engine or UI protocol layers.

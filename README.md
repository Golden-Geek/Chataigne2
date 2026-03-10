# Chataigne2

Chataigne2 is the desktop shell and integration layer for the Golden engine and UI stack.

This repository is being actively refactored toward a clean long-term architecture:

- `Chataigne2` stays a thin application shell.
- `golden_core` is being pushed toward pure engine and protocol responsibilities.
- UI state and transport boundaries are being normalized in `src-ui`.
- protocol declarations and code generation are moving toward a single source of truth.

## Repository Map

- `src/`: app-shell bootstrap, app-specific nodes, and shell wiring.
- `src-ui/`: Svelte 5 UI application and UI transport/client layers.
- `submodules/golden_core/`: shared engine, macros, and deeper design docs.
- `build.rs`: app node registry generation through a public codegen support crate.

## Start Here

- Read [ARCHITECTURE.md](ARCHITECTURE.md) for the top-level layer map.
- Read [CONTRIBUTING.md](CONTRIBUTING.md) for formatting, boundary, and codegen rules.
- Read [AGENTS.md](AGENTS.md) for the current repo operating rules and refactor direction.

## Existing Design Docs

The deeper engine design notes currently live under `submodules/golden_core/crates/core/docs/` and include:

- `dashboard_system.md`
- `node_blueprints.md`
- `node_contexts.md`
- `parameters_control_modes.md`
- `scripting_schema.md`

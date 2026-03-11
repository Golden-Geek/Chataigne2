# Chataigne2

Chataigne2 is the desktop shell and integration layer for the Golden engine and UI stack.

This repository is being actively refactored toward a clean long-term architecture:

- `Chataigne2` stays a thin application shell.
- `golden_core` provides explicit engine, protocol, persistence, transport, desktop-host, script, macro, and codegen crates behind a stable facade.
- `golden_ui` is treated as a reusable UI package boundary rather than app-local source ownership.
- Apps can override command-line parsing and bootstrap when needed, but they should not have to by default.
- UI state and transport boundaries are being normalized in `src-ui`.
- protocol declarations and code generation are moving toward a single source of truth.

## Repository Map

- `src/`: app-shell bootstrap, app-specific nodes, and minimal shell wiring.
- `src-ui/`: Svelte 5 app UI shell and `golden_ui` package consumption.
- `submodules/golden_core/`: shared engine, macros, and deeper design docs.
- `build.rs`: app node registry generation through a public codegen support crate.

## Start Here

- Read [ARCHITECTURE.md](ARCHITECTURE.md) for the top-level layer map.
- Read [CONTRIBUTING.md](CONTRIBUTING.md) for formatting, boundary, and codegen rules.
- Read [docs/repo-transition-plan.md](docs/repo-transition-plan.md) for current-vs-target repo ownership and migration rules.
- Read [AGENTS.md](AGENTS.md) for the current repo operating rules and refactor direction.

## Existing Design Docs

The deeper engine design notes currently live under `submodules/golden_core/crates/core/docs/` and include:

- `dashboard_system.md`
- `node_blueprints.md`
- `node_contexts.md`
- `parameters_control_modes.md`
- `scripting_schema.md`

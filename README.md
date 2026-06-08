# Chataigne2

Chataigne2 is the desktop shell and integration layer for the Golden engine and UI stack.

This repository is being actively refactored toward a clean long-term architecture:

- `Chataigne2` stays a thin application shell.
- `golden_core` provides explicit engine, protocol, persistence, transport, desktop-host, script, macro, and codegen crates behind a stable facade.
- `golden_ui` is treated as a reusable UI package boundary rather than app-local source ownership.
- Apps can override command-line parsing and bootstrap when needed, but they should not have to by default.
- UI state and transport boundaries are being normalized in `src-ui`.
- protocol declarations and code generation are moving toward a single source of truth.
- `cargo run` and the built app now ship the UI bundle and serve it from the built-in Rust host by default, so the desktop app does not depend on a separate Vite process to be usable.
- `cargo run -- --dev` now launches against the live Svelte/Vite dev server from `src-ui` instead of the bundled frontend, so frontend iteration can use the normal dev pipeline.
- `--no-frontend` disables bundled UI serving but still launches Tauri against an external frontend you start yourself.
- On Windows, debug builds keep their console output during `cargo run`, while release-style builds stay window-only unless `--show-output` is passed.

## First Clone Run

From a fresh clone, use the repo bootstrap command for your platform:

```powershell
.\tools\dev.ps1
```

```sh
bash ./tools/dev.sh
```

The bootstrap command initializes Git submodules, installs or verifies Rust through `rustup`, installs
or verifies Node.js/npm for the Svelte frontend, installs supported desktop build prerequisites, runs
`npm ci` in `src-ui` when needed, then runs `cargo run`.

On Windows it also selects the `stable-msvc` Rust toolchain, which Tauri needs for desktop builds.

After that first setup, `cargo run` is the normal launch command. The Rust build embeds the Svelte UI
bundle and will refresh `src-ui` dependencies when the package lock changes. For live frontend
iteration, pass app arguments through the bootstrap command or Cargo:

```sh
bash ./tools/dev.sh -- --dev
cargo run -- --dev
```

Use `.\tools\dev.ps1 -SetupOnly` or `bash ./tools/dev.sh --setup-only` when you only want to prepare
the machine without launching the app.

## Repository Map

- `src/`: app-shell bootstrap plus the app-owned node tree.
- `src/module/`: real Chataigne module foundation, including shared module roots/managers plus concrete module families under `src/module/modules/`.
- `src-ui/`: Svelte 5 app UI shell and `golden_ui` package consumption.
- `submodules/golden_core/`: shared engine, macros, and deeper design docs.
- `build.rs`: app node registry generation through a public codegen support crate.

## Start Here

- Read [ARCHITECTURE.md](ARCHITECTURE.md) for the top-level layer map.
- Read [docs/architecture.md](docs/architecture.md) for the contributor-facing boundary summary.
- Read [docs/STATE_MACHINE_ARCHITECTURE.md](docs/STATE_MACHINE_ARCHITECTURE.md) for Alchemist,
  statechart, Processor, intent, and UI ownership.
- Read [docs/contributor-map.md](docs/contributor-map.md) for practical ownership rules, generated
  files, and the intentional `noisette` project-file naming.
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

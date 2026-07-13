# Chataigne2

Chataigne2 is the desktop shell and integration layer for the Golden engine and UI stack.

This repository is being actively refactored toward a clean long-term architecture:

- `Chataigne2` stays a thin application shell.
- `golden_core` provides explicit engine, protocol, persistence, transport, desktop-host, script, macro, and codegen crates behind a stable facade.
- `golden_ui` is treated as a reusable UI package boundary rather than app-local source ownership.
- Apps can override command-line parsing and bootstrap when needed, but they should not have to by default.
- UI state and transport boundaries are being normalized in `apps/chataigne/ui` and the reusable
  packages under `packages/`.
- protocol declarations and code generation are moving toward a single source of truth.
- `cargo run` and the built app now ship the UI bundle and serve it from the built-in Rust host by default, so the desktop app does not depend on a separate Vite process to be usable.
- `cargo run -- --dev` launches against the live Svelte/Vite dev server from
  `apps/chataigne/ui` instead of the bundled frontend.
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

The bootstrap command installs the exact Rust host and checksum-verified portable Node.js/npm
distribution from `tools/bootstrap/toolchain.json`, verifies the supported Python version, installs
desktop build prerequisites, runs root `npm ci` when needed, then runs `cargo run`. No submodule
initialization is required. Install Git and the manifest's Python version first; the detailed host
prerequisites and cache behavior are in [docs/development.md](docs/development.md).

On Windows it selects the manifest's versioned MSVC host rather than floating `stable-msvc`.

After that first setup, `cargo run` is the normal launch command. The Rust build embeds the Svelte UI
bundle and will refresh workspace dependencies when the root package lock changes. For live frontend
iteration, pass app arguments through the bootstrap command or Cargo:

```sh
bash ./tools/dev.sh -- --dev
cargo run -- --dev
```

Use `.\tools\dev.ps1 -SetupOnly` or `bash ./tools/dev.sh --setup-only` when you only want to prepare
the machine without launching the app.

## Repository Map

- `apps/chataigne/`: executable app, app-owned modules, state machine, Tauri resources, formulas,
  and Svelte 5 UI.
- `crates/`: shared engine, protocol, persistence, host, transport, scripting, Alchemist, statechart,
  macros, and codegen crates.
- `packages/golden-ui/`: reusable application workbench and dock UI package.
- `packages/golden-alchemist-ui/`: reusable infinite node-graph canvas package.
- `Cargo.toml` and `package.json`: the single root Rust and JavaScript workspaces.

## Start Here

- Read [ARCHITECTURE.md](ARCHITECTURE.md) for the top-level layer map.
- Read [docs/architecture.md](docs/architecture.md) for the contributor-facing boundary summary.
- Read [docs/STATE_MACHINE_ARCHITECTURE.md](docs/STATE_MACHINE_ARCHITECTURE.md) for Alchemist,
  statechart, Processor, intent, and UI ownership.
- Read [docs/contributor-map.md](docs/contributor-map.md) for practical ownership rules, generated
  files, and the intentional `noisette` project-file naming.
- Read [docs/development.md](docs/development.md) for supported setup, root commands, diagnostics,
  dependency qualification, and cache policy.
- Read [CONTRIBUTING.md](CONTRIBUTING.md) for formatting, boundary, and codegen rules.
- Read [docs/repo-transition-plan.md](docs/repo-transition-plan.md) for current-vs-target repo ownership and migration rules.
- Read [AGENTS.md](AGENTS.md) for the current repo operating rules and refactor direction.

## Existing Design Docs

The deeper engine design notes live under `crates/core/docs/` and include:

- `dashboard_system.md`
- `node_blueprints.md`
- `node_contexts.md`
- `parameters_control_modes.md`
- `scripting_schema.md`

# Chataigne2

Chataigne2 is the desktop shell and product composition layer for the Golden engine and UI stack.
The repository is one self-contained Cargo/npm monorepo with these ownership boundaries:

- `Chataigne2` stays a thin application shell.
- `golden_application` defines replaceable project, graph, value, observation, module-I/O,
  persistence, and host-lifecycle contracts; the reusable crates under `crates/` implement the
  authoritative runtime, protocol, transport, desktop/headless host, persistence, script, macro,
  and codegen paths.
- `golden_model` owns stable cross-layer identities, `golden_values` owns the canonical runtime
  value model, `golden_parameters` owns parameter contracts and projections, and `golden_context`
  owns app-agnostic context declarations and state. The engine consumes these foundations rather
  than redeclaring them.
- `golden_ui`, `golden_graph_ui`, and `golden_statechart_ui` are reusable UI package boundaries.
- Chataigne-owned Alchemist lives under `apps/chataigne/alchemist`; its Formula UI lives under the
  app UI and consumes the public Golden graph canvas.
- Apps can override command-line parsing and bootstrap when needed, but they should not have to by default.
- Rust owns the public UI protocol and generates the TypeScript transport bindings.

Start with [Application Seams and Phase 2 Shadowing](docs/architecture/application-seams.md) for
the migration boundary and its side-effect-safety rules.
[Foundation Ownership](docs/architecture/foundations.md) describes the Phase 3 identity, value,
parameter, and context cutovers that graph packages build upon.
[Graph Foundation](docs/architecture/graph-foundation.md) describes the typed graph-domain,
transaction, revision, topology, presentation, traversal, and persistence contract.
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
- `packages/golden-graph-ui/`: reusable infinite node-graph canvas package.
- `packages/golden-statechart-ui/`: reusable statechart canvas projection.
- `Cargo.toml` and `package.json`: the single root Rust and JavaScript workspaces.

## Start Here

- Read [ARCHITECTURE.md](ARCHITECTURE.md) for the top-level layer map.
- Read [docs/architecture.md](docs/architecture.md) for the contributor-facing boundary summary.
- Read [Statecharts, Conditions, and Processors](docs/architecture/statecharts-conditions-processors.md)
  for Alchemist, statechart, Processor, intent, and UI ownership.
- Read [docs/contributor-map.md](docs/contributor-map.md) for practical ownership rules, generated
  files, and the intentional `noisette` project-file naming.
- Read [docs/development.md](docs/development.md) for supported setup, root commands, diagnostics,
  dependency qualification, and cache policy.
- Read [docs/module-authoring.md](docs/module-authoring.md) and
  [docs/ui-extension.md](docs/ui-extension.md) before adding product modules or reusable UI hooks.
- Read [CONTRIBUTING.md](CONTRIBUTING.md) for formatting, boundary, and codegen rules.
- Read [docs/performance.md](docs/performance.md) for runtime and UI scale contracts.
- Read [docs/troubleshooting.md](docs/troubleshooting.md) for build, launch, connection, and package diagnosis.
- Read [AGENTS.md](AGENTS.md) for the current repo operating rules and refactor direction.

## Existing Design Docs

The deeper engine design notes live under `crates/core/docs/` and include:

- `dashboard_system.md`
- `node_blueprints.md`
- `node_contexts.md`
- `parameters_control_modes.md`
- `scripting_schema.md`

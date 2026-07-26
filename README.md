# Chataigne2

Chataigne2 is the desktop shell and product composition layer for the Golden engine and UI stack.
The repository is one self-contained Cargo/npm monorepo with these ownership boundaries:

- `Chataigne2` stays a thin application shell.
- `crates/golden_core/` contains the public facade plus grouped engine, foundation, runtime,
  service, host, and build-support crates.
- `golden_model` owns stable cross-layer identities, `golden_values` owns the canonical runtime
  value model, `golden_parameters` owns parameter contracts and projections, and `golden_context`
  owns app-agnostic context declarations and state. The engine consumes these foundations rather
  than redeclaring them.
- `golden_graph` and `golden_graph_ui` are the reusable graph document and canvas boundaries.
- `golden_ui` is the reusable workbench boundary.
- Chataigne-owned Alchemist lives under `apps/chataigne/systems/alchemist` and includes formulas,
  conditions, processors, Inputs, Filters, and Outputs. Chataigne's state-machine model and runtime
  live beside it under `apps/chataigne/systems/state_machine`.
- Apps can override command-line parsing and bootstrap when needed, but they should not have to by default.
- Rust owns the public UI protocol and generates the TypeScript transport bindings.

Start with [Application Contracts](docs/architecture/application-contracts.md) for the reusable
runtime boundary and its side-effect-safety rules.
[Foundation Ownership](docs/architecture/foundations.md) describes the identity, value, parameter,
and context contracts that graph packages build upon.
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
prerequisites and cache behavior are in [docs/guides/development.md](docs/guides/development.md).

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

- `apps/chataigne/`: executable shell, product modules, Alchemist, state machine, resources, and UI.
- `crates/golden_core/`: reusable backend framework, grouped by responsibility.
- `crates/golden_graph/`: reusable graph document and transaction system.
- `packages/golden-ui/`: reusable application workbench and dock UI package.
- `packages/golden-audio-ui/`: reusable audio device inspector and generated audio UI contracts.
- `packages/golden-graph-ui/`: reusable infinite node-graph canvas package.
- `Cargo.toml` and `package.json`: the single root Rust and JavaScript workspaces.

See [Repository Layout](docs/reference/repository-layout.md) for the complete tree.

## Start Here

- Read [ARCHITECTURE.md](ARCHITECTURE.md) for the top-level layer map.
- Use the [documentation index](docs/README.md) to find focused design and workflow pages.
- Read [docs/architecture/README.md](docs/architecture/README.md) for the contributor-facing boundary summary.
- Read [Statecharts, Conditions, and Processors](docs/architecture/statecharts-conditions-processors.md)
  for Alchemist, statechart, Processor, intent, and UI ownership.
- Read [docs/reference/contributor-map.md](docs/reference/contributor-map.md) for practical ownership rules, generated
  files, and the intentional `noisette` project-file naming.
- Read [docs/guides/development.md](docs/guides/development.md) for supported setup, root commands, diagnostics,
  dependency qualification, and cache policy.
- Read [docs/guides/module-authoring.md](docs/guides/module-authoring.md) and
  [docs/guides/ui-extension.md](docs/guides/ui-extension.md) before adding product modules or reusable UI hooks.
- Read [CONTRIBUTING.md](CONTRIBUTING.md) for formatting, boundary, and codegen rules.
- Read [docs/operations/performance.md](docs/operations/performance.md) for runtime and UI scale contracts.
- Read [docs/operations/troubleshooting.md](docs/operations/troubleshooting.md) for build, launch, connection, and package diagnosis.
- Read [AGENTS.md](AGENTS.md) for the current repo operating rules and refactor direction.

## Engine Design Notes

The maintained engine notes live under `crates/golden_core/engine/docs/`:

- `dashboard_system.md`
- `source_layout.md`

## License

Chataigne2 and the reusable Golden workspace packages are licensed under the
[GNU General Public License version 3](LICENSE), identified in package metadata as
`GPL-3.0-only`.

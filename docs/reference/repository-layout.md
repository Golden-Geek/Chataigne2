# Repository Layout

The root `Cargo.toml`, `Cargo.lock`, `package.json`, and `package-lock.json` define one Rust
and npm workspace.

## Backend

```text
crates/
├── golden_core/                 public facade and reusable backend framework
│   ├── engine/                  node tree, engine loop, scheduling, UI read model
│   ├── foundation/
│   │   ├── application/         application contracts
│   │   ├── context/             context declarations and state
│   │   ├── model/               cross-layer identities
│   │   ├── parameters/          parameter contracts and projections
│   │   └── values/              canonical runtime values
│   ├── runtime/
│   │   ├── control/             compiled control/runtime primitives
│   │   ├── io/                  workers, queues, and recovery
│   │   └── script/              generic scripting integration
│   ├── services/
│   │   ├── persistence/         project/file persistence
│   │   └── protocol/            public UI protocol
│   ├── hosts/
│   │   ├── desktop/             Tauri and native dialogs
│   │   └── transport/           HTTP/WebSocket host
│   └── support/
│       ├── codegen/             supported build/codegen API
│       └── macros/              proc macros
└── golden_graph/                app-agnostic graph documents and transactions
```

## Chataigne

```text
apps/chataigne/
├── src/
│   ├── app/                     thin lifecycle and composition shell
│   └── module/                  concrete product modules
├── tests/
│   └── samples/                 crate-level project fixtures
├── systems/
│   ├── alchemist/
│   │   ├── src/                 Formula/ANode graph, compiler, and runtime
│   │   ├── condition/           condition authoring/compiler/runtime
│   │   ├── processor/           Inputs, Filters, Outputs, lanes, and ValueSet
│   │   └── integration/         Golden Core nodes for the Alchemist surface
│   └── state_machine/
│       ├── model/               graph-backed state/transition model
│       ├── runtime/             execution, arbitration, and protocol
│       └── integration/         Golden Core state/transition nodes
├── resources/
│   └── formulas/builtin/        shipped Action and Mapping formula assets
└── ui/                          Svelte 5 product UI
```

`apps/chataigne/build.rs` generates the app node registry and embeds bundled UI/formula assets.
Desktop capabilities, icons, permissions, and Tauri configuration remain directly under the app.

Within every Rust crate and UI source area, unit tests live in a local `tests/` directory directly
under the feature they exercise. A crate's top-level `tests/` directory is reserved for crate-wide
integration tests and their fixtures. See [Development Workflows](../guides/development.md#test-placement)
for the exact convention.

## UI packages

- `packages/golden-ui/`: reusable workbench, stores, transport adapters, and generated Rust
  protocol bindings.
- `packages/golden-audio-ui/`: reusable audio device selection/status UI and generated
  `golden_audio` contracts.
- `packages/golden-graph-ui/`: domain-neutral graph canvas.
- `apps/chataigne/ui/src/lib/systems/alchemist/`: Formula, condition, processor, input, filter, and
  output UI.
- `apps/chataigne/ui/src/lib/systems/state_machine/`: State Machine panel, local graph projection,
  and generated protocol bundle.

## Documentation and tooling

- `docs/architecture/`: ownership and dependency design.
- `docs/guides/`: contributor workflows.
- `docs/operations/`: performance, release, troubleshooting, and hygiene runbooks.
- `docs/reference/`: exact maps and policies.
- `tools/`: bootstrap, checks, qualification, product gates, and release tooling.
- `xtask/`: root watch orchestration.

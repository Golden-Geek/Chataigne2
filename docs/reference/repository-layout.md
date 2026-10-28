# Repository Layout

The root `Cargo.toml`, `Cargo.lock`, `package.json`, and `package-lock.json` define one Rust
and npm workspace.

## Backend

`crates/golden_audio/` is a sibling reusable foundation: it owns the app-agnostic audio engine,
native host adapters, playback, analysis, and qualification harnesses. It does not depend on
Golden Core or Chataigne.

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
The persistent Sound Card model and product policy live under
`apps/chataigne/src/module/modules/audio/sound_card/`; reusable device/status presentation lives in
`packages/golden-audio-ui/`.

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

## Planned MapGyver boundaries

These paths describe the proposed design, not existing workspace members. See the
[MapGyver architecture](../architecture/mapgyver.md) and [plan](../plan/MapGyver_Implementation_Plan.md).

- `apps/mapgyver/src/app/`: thin Golden app shell and product registration.
- `apps/mapgyver/systems/{media,screen,matrix}/`: app-owned models, backend operations and integration.
- `apps/mapgyver/ui/`: Screen, Surface Source, Composition and Matrix editors.
- `crates/golden_video/`: reusable GPU runtime, compositor primitives, Spout and presentation.
- `packages/golden-video-ui/`: reusable native-preview bindings; host integration remains shared.

## Documentation and tooling

- `docs/architecture/`: ownership and dependency design.
- `docs/guides/`: contributor workflows.
- `docs/operations/`: performance, release, troubleshooting, and hygiene runbooks.
- `docs/reference/`: exact maps and policies.
- `docs/plan/`: product implementation plans; root `AGENTS.md` selects Chataigne by default and
  MapGyver only when explicitly requested.
- `docs/progress/`: current product ledgers and qualification evidence.
- `docs/archive/`: non-operative historical plans and qualification evidence.
- `tools/`: bootstrap, checks, qualification, product gates, and release tooling.
- `xtask/`: root watch orchestration.
